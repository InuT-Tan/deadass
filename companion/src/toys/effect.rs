use crate::haptics::HapticCommand;
use deadass_shared::Pattern;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;
use tokio::sync::Mutex as AsyncMutex;
use tokio::task::AbortHandle;

const PULSE_COUNT: u64 = 3;
const RAMP_STEPS: [f64; 3] = [0.4, 0.7, 1.0];
/// buttplug_client puts no timeout on device writes, so a half-open connection would
/// otherwise hold the write turn forever and starve every other toy.
const WRITE_TIMEOUT: Duration = Duration::from_millis(500);

/// A toy output that takes a vibration level in `0.0..=1.0`.
pub trait Output: Clone + PartialEq + Send + Sync + 'static {
    fn set_level(&self, level: f64) -> impl Future<Output = ()> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayOutcome {
    Started,
    /// A stronger effect is still playing; this one was dropped.
    Outranked,
}

/// Plays one effect at a time. A new effect interrupts the one in progress if it is
/// at least as strong; weaker effects are dropped while a stronger one plays.
///
/// Every device write passes through one lock and is checked against the generation
/// of the effect that issued it, so an interrupted effect cannot land a write after
/// its successor's first write, even from a poll already running on another worker.
pub struct EffectSlot<D: Output> {
    shared: Arc<Shared<D>>,
}

struct Shared<D> {
    generation: AtomicU64,
    turn: AsyncMutex<()>,
    current: Mutex<Option<Running<D>>>,
}

struct Running<D> {
    generation: u64,
    strength: f64,
    abort: AbortHandle,
    /// Its targets plus the toys it was meant to release: if it is interrupted
    /// before it runs, the next effect inherits the job of stopping them.
    responsible_for: Vec<D>,
}

impl<D: Output> Default for EffectSlot<D> {
    fn default() -> Self {
        Self {
            shared: Arc::new(Shared {
                generation: AtomicU64::new(0),
                turn: AsyncMutex::new(()),
                current: Mutex::new(None),
            }),
        }
    }
}

impl<D: Output> EffectSlot<D> {
    pub fn play(&self, command: HapticCommand, targets: Vec<D>) -> PlayOutcome {
        let current = self.shared.current();
        let outranked = current.as_ref().is_some_and(|running| {
            !running.abort.is_finished() && command.strength < running.strength
        });
        if outranked {
            return PlayOutcome::Outranked;
        }
        self.launch(current, Some(command), targets);
        PlayOutcome::Started
    }

    /// Interrupts whatever is playing and turns off every toy it was driving.
    pub fn stop(&self) {
        let current = self.shared.current();
        self.launch(current, None, Vec::new());
    }

    fn launch(
        &self,
        mut current: MutexGuard<'_, Option<Running<D>>>,
        command: Option<HapticCommand>,
        targets: Vec<D>,
    ) {
        let generation = self.shared.generation.fetch_add(1, Ordering::SeqCst) + 1;
        // Toys the interrupted effect was driving but this one is not would
        // otherwise keep vibrating at whatever level they were left on.
        let released: Vec<D> = match current.take() {
            Some(previous) => {
                previous.abort.abort();
                previous
                    .responsible_for
                    .into_iter()
                    .filter(|device| !targets.contains(device))
                    .collect()
            }
            None => Vec::new(),
        };
        let mut responsible_for = targets.clone();
        responsible_for.extend(released.iter().cloned());
        let writer = Writer {
            shared: Arc::clone(&self.shared),
            generation,
        };
        let task = tokio::spawn(async move {
            writer.set(&released, 0.0).await;
            if let Some(command) = command {
                run(&writer, &targets, command).await;
            }
            writer.finish();
        });
        *current = Some(Running {
            generation,
            strength: command.map_or(f64::NEG_INFINITY, |command| command.strength),
            abort: task.abort_handle(),
            responsible_for,
        });
    }
}

impl<D> Shared<D> {
    fn current(&self) -> MutexGuard<'_, Option<Running<D>>> {
        // The guarded state is valid between statements, so a panic elsewhere
        // must not wedge every later effect.
        self.current.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

struct Writer<D> {
    shared: Arc<Shared<D>>,
    generation: u64,
}

impl<D: Output> Writer<D> {
    async fn set(&self, devices: &[D], level: f64) {
        if devices.is_empty() {
            return;
        }
        let _turn = self.shared.turn.lock().await;
        for device in devices {
            if self.shared.generation.load(Ordering::SeqCst) != self.generation {
                return;
            }
            let _ = tokio::time::timeout(WRITE_TIMEOUT, device.set_level(level)).await;
        }
    }

    /// A cleanly finished effect left its toys at 0: drop its release obligation
    /// so later effects do not keep re-zeroing them, and stop it outranking anything.
    fn finish(&self) {
        let mut current = self.shared.current();
        if current
            .as_ref()
            .is_some_and(|running| running.generation == self.generation)
        {
            *current = None;
        }
    }
}

async fn run<D: Output>(writer: &Writer<D>, targets: &[D], command: HapticCommand) {
    let strength = command.strength.clamp(0.0, 1.0);
    match command.pattern {
        Pattern::Vibrate => {
            writer.set(targets, strength).await;
            if command.duration_ms > 0 {
                sleep_ms(command.duration_ms).await;
                writer.set(targets, 0.0).await;
            }
        }
        Pattern::Pulse => {
            let each = command.duration_ms / (PULSE_COUNT * 2);
            for _ in 0..PULSE_COUNT {
                writer.set(targets, strength).await;
                sleep_ms(each).await;
                writer.set(targets, 0.0).await;
                sleep_ms(each).await;
            }
        }
        Pattern::Ramp => {
            let each = command.duration_ms / RAMP_STEPS.len() as u64;
            for step in RAMP_STEPS {
                writer.set(targets, strength * step).await;
                sleep_ms(each).await;
            }
            writer.set(targets, 0.0).await;
        }
    }
}

async fn sleep_ms(duration_ms: u64) {
    tokio::time::sleep(Duration::from_millis(duration_ms)).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::time::Instant;

    type Log = Arc<Mutex<Vec<(char, f64)>>>;

    /// `hang` never completes a write. `stall` makes the 0.8 write to `b` spin inside
    /// the same poll, after the generation check and before the write lands, until
    /// released: the window a write-turn lock has to cover.
    #[derive(Clone)]
    struct Fake {
        id: char,
        log: Log,
        hang: bool,
        stall: Option<Arc<Stall>>,
    }

    #[derive(Default)]
    struct Stall {
        spinning: AtomicBool,
        release: AtomicBool,
    }

    impl PartialEq for Fake {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }

    impl Output for Fake {
        fn set_level(&self, level: f64) -> impl Future<Output = ()> + Send {
            let fake = self.clone();
            async move {
                if fake.hang {
                    std::future::pending::<()>().await;
                }
                if let Some(stall) = fake.stall.filter(|_| fake.id == 'b' && level == 0.8) {
                    stall.spinning.store(true, Ordering::SeqCst);
                    let started = Instant::now();
                    while !stall.release.load(Ordering::SeqCst)
                        && started.elapsed() < Duration::from_millis(500)
                    {
                        std::hint::spin_loop();
                    }
                }
                fake.log.lock().unwrap().push((fake.id, level));
            }
        }
    }

    fn toys(ids: &str) -> (Vec<Fake>, Log) {
        let log = Log::default();
        let devices = ids
            .chars()
            .map(|id| Fake {
                id,
                log: Arc::clone(&log),
                hang: false,
                stall: None,
            })
            .collect();
        (devices, log)
    }

    fn command(pattern: Pattern, strength: f64, duration_ms: u64) -> HapticCommand {
        HapticCommand {
            strength,
            duration_ms,
            pattern,
        }
    }

    fn buzz(strength: f64, duration_ms: u64) -> HapticCommand {
        command(Pattern::Vibrate, strength, duration_ms)
    }

    fn take(log: &Log) -> Vec<(char, f64)> {
        std::mem::take(&mut *log.lock().unwrap())
    }

    async fn advance(ms: u64) {
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }

    #[tokio::test(start_paused = true)]
    async fn patterns_keep_their_shape() {
        let (devices, log) = toys("a");
        let slot = EffectSlot::default();
        slot.play(command(Pattern::Ramp, 1.0, 300), devices.clone());
        advance(400).await;
        assert_eq!(take(&log), [('a', 0.4), ('a', 0.7), ('a', 1.0), ('a', 0.0)]);

        // The finished ramp no longer outranks a weaker effect.
        slot.play(command(Pattern::Pulse, 0.5, 600), devices);
        advance(700).await;
        assert_eq!(take(&log), [('a', 0.5), ('a', 0.0)].repeat(3));
    }

    #[tokio::test(start_paused = true)]
    async fn stronger_or_equal_interrupts_and_the_old_one_never_writes_again() {
        let (devices, log) = toys("a");
        let slot = EffectSlot::default();
        slot.play(buzz(0.3, 1000), devices.clone());
        advance(50).await;
        assert_eq!(
            slot.play(buzz(0.3, 1000), devices.clone()),
            PlayOutcome::Started
        );
        advance(50).await;
        assert_eq!(slot.play(buzz(0.8, 100), devices), PlayOutcome::Started);
        advance(2000).await;
        // No trailing 0.0 at t=1000 or t=1050 from the interrupted holds.
        assert_eq!(take(&log), [('a', 0.3), ('a', 0.3), ('a', 0.8), ('a', 0.0)]);
    }

    #[tokio::test(start_paused = true)]
    async fn weaker_effect_is_dropped_while_a_stronger_one_plays() {
        let (devices, log) = toys("a");
        let slot = EffectSlot::default();
        slot.play(buzz(0.8, 1000), devices.clone());
        advance(50).await;
        assert_eq!(
            slot.play(buzz(0.3, 100), devices.clone()),
            PlayOutcome::Outranked
        );
        advance(2000).await;
        assert_eq!(take(&log), [('a', 0.8), ('a', 0.0)]);
        assert_eq!(slot.play(buzz(0.3, 100), devices), PlayOutcome::Started);
    }

    #[tokio::test(start_paused = true)]
    async fn toys_the_new_effect_does_not_use_are_stopped() {
        let (devices, log) = toys("ab");
        let slot = EffectSlot::default();
        slot.play(buzz(0.3, 1000), devices.clone());
        advance(50).await;
        slot.play(buzz(0.8, 100), devices[..1].to_vec());
        advance(2000).await;
        assert_eq!(
            take(&log),
            [('a', 0.3), ('b', 0.3), ('b', 0.0), ('a', 0.8), ('a', 0.0)]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn release_survives_an_effect_interrupted_before_it_ran() {
        let (devices, log) = toys("ab");
        let slot = EffectSlot::default();
        slot.play(buzz(0.3, 1000), devices.clone());
        advance(50).await;
        // Back-to-back events from one console.log chunk: the second effect is
        // superseded before its task ever runs, so it never stops `b` itself.
        slot.play(buzz(0.5, 100), devices[..1].to_vec());
        slot.play(buzz(0.8, 100), devices[..1].to_vec());
        advance(2000).await;
        assert_eq!(
            take(&log),
            [('a', 0.3), ('b', 0.3), ('b', 0.0), ('a', 0.8), ('a', 0.0)]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_cleanly_finished_effect_is_not_released_again() {
        let (devices, log) = toys("ab");
        let slot = EffectSlot::default();
        slot.play(buzz(0.5, 100), devices[1..].to_vec());
        advance(200).await;
        take(&log);
        slot.play(buzz(0.5, 100), devices[..1].to_vec());
        advance(200).await;
        assert_eq!(take(&log), [('a', 0.5), ('a', 0.0)]);
    }

    #[tokio::test(start_paused = true)]
    async fn stop_turns_off_every_toy_the_effect_drove() {
        let (devices, log) = toys("ab");
        let slot = EffectSlot::default();
        slot.play(command(Pattern::Pulse, 0.8, 3000), devices.clone());
        advance(50).await;
        slot.stop();
        advance(3000).await;
        assert_eq!(take(&log), [('a', 0.8), ('b', 0.8), ('a', 0.0), ('b', 0.0)]);
    }

    #[tokio::test(start_paused = true)]
    async fn stop_obligation_survives_an_immediate_new_effect() {
        let (devices, log) = toys("ab");
        let slot = EffectSlot::default();
        slot.play(buzz(0.8, 3000), devices.clone());
        advance(50).await;
        slot.stop();
        // Anything outranks a stop, even before its zero-writes have run.
        assert_eq!(
            slot.play(buzz(0.1, 100), devices[..1].to_vec()),
            PlayOutcome::Started
        );
        advance(1000).await;
        assert_eq!(
            take(&log),
            [('a', 0.8), ('b', 0.8), ('b', 0.0), ('a', 0.1), ('a', 0.0)]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn writes_from_a_superseded_generation_are_dropped() {
        let (devices, log) = toys("a");
        let slot: EffectSlot<Fake> = EffectSlot::default();
        let stale = Writer {
            shared: Arc::clone(&slot.shared),
            generation: 1,
        };
        slot.shared.generation.store(2, Ordering::SeqCst);
        stale.set(&devices, 0.9).await;
        assert!(take(&log).is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn a_hung_device_write_does_not_starve_healthy_toys() {
        let (mut devices, log) = toys("xy");
        devices[0].hang = true;
        let slot = EffectSlot::default();
        slot.play(buzz(0.8, 2000), devices.clone());
        advance(WRITE_TIMEOUT.as_millis() as u64 + 50).await;
        assert_eq!(take(&log), [('y', 0.8)]);
        slot.play(buzz(0.9, 100), devices[1..].to_vec());
        advance(3 * WRITE_TIMEOUT.as_millis() as u64).await;
        assert_eq!(take(&log), [('y', 0.9), ('y', 0.0)]);
    }

    #[tokio::test(start_paused = true)]
    async fn an_interrupted_effect_task_is_ended_not_left_sleeping() {
        let (devices, _log) = toys("a");
        let slot = EffectSlot::default();
        slot.play(buzz(0.5, 60_000), devices.clone());
        advance(10).await;
        let first = slot.shared.current().as_ref().unwrap().abort.clone();
        slot.play(buzz(0.9, 100), devices);
        advance(1).await;
        assert!(first.is_finished());
    }

    /// Without the write-turn lock, the successor can write while the interrupted
    /// effect sits between its generation check and its write, and the stale 0.8 lands last.
    #[test]
    fn a_write_already_past_its_check_lands_before_the_successor_writes() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let (mut devices, log) = toys("ab");
            let stall = Arc::new(Stall::default());
            devices[1].stall = Some(Arc::clone(&stall));
            let slot = EffectSlot::default();
            slot.play(buzz(0.8, 5000), devices.clone());
            while !stall.spinning.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            slot.play(buzz(0.9, 50), devices[1..].to_vec());
            tokio::time::sleep(Duration::from_millis(30)).await; // successor's chance to jump in
            stall.release.store(true, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(200)).await;
            let seen = log.lock().unwrap().clone();
            assert_eq!(
                seen,
                [('a', 0.8), ('b', 0.8), ('a', 0.0), ('b', 0.9), ('b', 0.0)]
            );
        });
    }
}
