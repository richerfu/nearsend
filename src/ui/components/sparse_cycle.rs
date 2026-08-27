use gpui::{Context, Task};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SparseCyclePhase {
    FadeIn,
    Stable,
    FadeOut,
}

#[derive(Clone, Copy)]
struct SparseCycleCursor {
    index: usize,
    item_count: usize,
    phase: SparseCyclePhase,
}

impl SparseCycleCursor {
    fn new(item_count: usize) -> Self {
        Self {
            index: 0,
            item_count: item_count.max(1),
            phase: SparseCyclePhase::FadeIn,
        }
    }

    fn advance(&mut self) {
        match self.phase {
            SparseCyclePhase::FadeIn => self.phase = SparseCyclePhase::Stable,
            SparseCyclePhase::Stable => self.phase = SparseCyclePhase::FadeOut,
            SparseCyclePhase::FadeOut => {
                self.index = (self.index + 1) % self.item_count;
                self.phase = SparseCyclePhase::FadeIn;
            }
        }
    }
}

pub(crate) struct SparseCycleState {
    cursor: SparseCycleCursor,
    transition_duration: Duration,
    _cycle_task: Task<()>,
}

impl SparseCycleState {
    pub(crate) fn new(
        item_count: usize,
        item_duration: Duration,
        transition_duration: Duration,
        cx: &mut Context<Self>,
    ) -> Self {
        let item_duration = item_duration.max(Duration::from_millis(2));
        let transition_duration = transition_duration
            .max(Duration::from_millis(1))
            .min(item_duration / 2);
        let stable_duration = item_duration.saturating_sub(transition_duration.saturating_mul(2));

        let cycle_task = if item_count > 1 {
            cx.spawn(async move |this, cx| loop {
                cx.background_executor().timer(transition_duration).await;
                if this
                    .update(cx, |state, cx| {
                        state.cursor.advance();
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }

                cx.background_executor().timer(stable_duration).await;
                if this
                    .update(cx, |state, cx| {
                        state.cursor.advance();
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }

                cx.background_executor().timer(transition_duration).await;
                if this
                    .update(cx, |state, cx| {
                        state.cursor.advance();
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }
            })
        } else {
            Task::ready(())
        };

        Self {
            cursor: SparseCycleCursor::new(item_count),
            transition_duration,
            _cycle_task: cycle_task,
        }
    }

    pub(crate) fn index(&self) -> usize {
        self.cursor.index
    }

    pub(crate) fn phase(&self) -> SparseCyclePhase {
        self.cursor.phase
    }

    pub(crate) fn transition_duration(&self) -> Duration {
        self.transition_duration
    }
}

#[cfg(test)]
mod tests {
    use super::{SparseCycleCursor, SparseCyclePhase};

    #[test]
    fn advances_only_after_each_visual_phase() {
        let mut cursor = SparseCycleCursor::new(2);
        assert_eq!(cursor.index, 0);
        assert_eq!(cursor.phase, SparseCyclePhase::FadeIn);

        cursor.advance();
        assert_eq!(cursor.index, 0);
        assert_eq!(cursor.phase, SparseCyclePhase::Stable);

        cursor.advance();
        assert_eq!(cursor.index, 0);
        assert_eq!(cursor.phase, SparseCyclePhase::FadeOut);

        cursor.advance();
        assert_eq!(cursor.index, 1);
        assert_eq!(cursor.phase, SparseCyclePhase::FadeIn);
    }
}
