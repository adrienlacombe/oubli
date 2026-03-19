use std::collections::VecDeque;

/// An operation waiting to be processed.
#[derive(Debug, Clone)]
pub struct QueuedOperation {
    pub id: u64,
    pub action: crate::actions::UserAction,
}

/// FIFO operation queue with sequential processing.
///
/// Only one operation runs at a time. When the head completes,
/// the next operation starts (rollover at head).
#[derive(Debug)]
pub struct OperationQueue {
    queue: VecDeque<QueuedOperation>,
    next_id: u64,
}

impl OperationQueue {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            next_id: 1,
        }
    }

    /// Enqueue an action. Returns the assigned operation ID.
    pub fn enqueue(&mut self, action: crate::actions::UserAction) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.queue.push_back(QueuedOperation { id, action });
        id
    }

    /// Peek at the head (currently processing) operation.
    pub fn current(&self) -> Option<&QueuedOperation> {
        self.queue.front()
    }

    /// Remove the head operation (it has completed).
    pub fn complete_current(&mut self) -> Option<QueuedOperation> {
        self.queue.pop_front()
    }

    /// Number of operations waiting (including the current one).
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Clear all pending operations.
    pub fn clear(&mut self) {
        self.queue.clear();
    }
}

impl Default for OperationQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::UserAction;

    #[test]
    fn fifo_ordering() {
        let mut q = OperationQueue::new();
        let id1 = q.enqueue(UserAction::Rollover);
        let id2 = q.enqueue(UserAction::Lock);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(q.len(), 2);

        let current = q.current().unwrap();
        assert_eq!(current.id, 1);

        q.complete_current();
        assert_eq!(q.current().unwrap().id, 2);
        assert_eq!(q.len(), 1);

        q.complete_current();
        assert!(q.is_empty());
    }

    #[test]
    fn clear_empties() {
        let mut q = OperationQueue::new();
        q.enqueue(UserAction::Rollover);
        q.enqueue(UserAction::Rollover);
        q.clear();
        assert!(q.is_empty());
    }

    #[test]
    fn ids_monotonic() {
        let mut q = OperationQueue::new();
        let a = q.enqueue(UserAction::Lock);
        let b = q.enqueue(UserAction::Lock);
        let c = q.enqueue(UserAction::Lock);
        assert!(a < b);
        assert!(b < c);
    }
}
