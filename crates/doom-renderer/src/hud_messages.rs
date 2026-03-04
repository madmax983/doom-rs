//! HUD message queue — timed text overlays for pickup notifications,
//! level names, and other on-screen messages.
//!
//! Doom displays messages at the top of the screen (e.g. "Picked up a
//! shotgun", "A secret is revealed!").  Messages expire after a fixed
//! duration in tics and are evicted when the queue reaches capacity.

/// A timed message displayed at the top of the screen.
#[derive(Clone, Debug)]
pub struct HudMessage {
    /// The text content of the message.
    text: String,
    /// Remaining display duration in tics.
    remaining_tics: u32,
}

impl HudMessage {
    /// Create a new HUD message.
    pub fn new(text: String, duration_tics: u32) -> Self {
        Self {
            text,
            remaining_tics: duration_tics,
        }
    }

    /// The text content of this message.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Remaining display duration in tics.
    pub fn remaining_tics(&self) -> u32 {
        self.remaining_tics
    }

    /// Whether this message has expired.
    pub fn is_expired(&self) -> bool {
        self.remaining_tics == 0
    }
}

/// Manages the HUD message queue (pickup notifications, level names, etc.).
///
/// Messages are displayed in order of insertion.  When the queue reaches
/// its maximum capacity, the oldest message is evicted to make room.
/// Each tic, all message durations are decremented and expired messages
/// are removed.
#[derive(Clone, Debug)]
pub struct HudMessageQueue {
    /// Currently active messages, oldest first.
    messages: Vec<HudMessage>,
    /// Maximum number of simultaneously visible messages.
    max_messages: usize,
}

impl HudMessageQueue {
    /// Create a new message queue with the given capacity.
    ///
    /// Typically 4 messages maximum for the classic Doom HUD.
    pub fn new(max_messages: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_messages,
        }
    }

    /// Push a new message onto the queue.
    ///
    /// If the queue is at capacity, the oldest message is evicted first.
    pub fn push(&mut self, text: String, duration_tics: u32) {
        // Evict oldest if at capacity.
        while self.messages.len() >= self.max_messages {
            self.messages.remove(0);
        }
        self.messages.push(HudMessage::new(text, duration_tics));
    }

    /// Advance all messages by one tic.
    ///
    /// Decrements `remaining_tics` for each message and removes any that
    /// have expired (reached 0).
    pub fn tick(&mut self) {
        for msg in &mut self.messages {
            if msg.remaining_tics > 0 {
                msg.remaining_tics -= 1;
            }
        }
        self.messages.retain(|msg| msg.remaining_tics > 0);
    }

    /// Return the currently visible messages (oldest first).
    pub fn active_messages(&self) -> &[HudMessage] {
        &self.messages
    }

    /// Remove all messages immediately.
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// Current number of active messages.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_queue_is_empty() {
        let queue = HudMessageQueue::new(4);
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
        assert!(queue.active_messages().is_empty());
    }

    #[test]
    fn push_adds_message() {
        let mut queue = HudMessageQueue::new(4);
        queue.push("Picked up a shotgun.".to_string(), 35);
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.active_messages()[0].text(), "Picked up a shotgun.");
        assert_eq!(queue.active_messages()[0].remaining_tics(), 35);
    }

    #[test]
    fn push_multiple_messages() {
        let mut queue = HudMessageQueue::new(4);
        queue.push("Message 1".to_string(), 10);
        queue.push("Message 2".to_string(), 20);
        queue.push("Message 3".to_string(), 30);
        assert_eq!(queue.len(), 3);
        assert_eq!(queue.active_messages()[0].text(), "Message 1");
        assert_eq!(queue.active_messages()[1].text(), "Message 2");
        assert_eq!(queue.active_messages()[2].text(), "Message 3");
    }

    #[test]
    fn tick_decrements_remaining_tics() {
        let mut queue = HudMessageQueue::new(4);
        queue.push("Test".to_string(), 5);
        queue.tick();
        assert_eq!(queue.active_messages()[0].remaining_tics(), 4);
    }

    #[test]
    fn tick_removes_expired_messages() {
        let mut queue = HudMessageQueue::new(4);
        queue.push("Short".to_string(), 1);
        queue.push("Long".to_string(), 100);

        queue.tick(); // Short goes to 0, Long goes to 99.
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.active_messages()[0].text(), "Long");
        assert_eq!(queue.active_messages()[0].remaining_tics(), 99);
    }

    #[test]
    fn max_messages_enforced_oldest_evicted() {
        let mut queue = HudMessageQueue::new(2);
        queue.push("First".to_string(), 100);
        queue.push("Second".to_string(), 100);
        assert_eq!(queue.len(), 2);

        // Pushing a third should evict the oldest ("First").
        queue.push("Third".to_string(), 100);
        assert_eq!(queue.len(), 2);
        assert_eq!(queue.active_messages()[0].text(), "Second");
        assert_eq!(queue.active_messages()[1].text(), "Third");
    }

    #[test]
    fn clear_removes_all() {
        let mut queue = HudMessageQueue::new(4);
        queue.push("A".to_string(), 100);
        queue.push("B".to_string(), 200);
        queue.push("C".to_string(), 300);
        assert_eq!(queue.len(), 3);

        queue.clear();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
        assert!(queue.active_messages().is_empty());
    }

    #[test]
    fn active_messages_returns_current() {
        let mut queue = HudMessageQueue::new(4);
        queue.push("Alpha".to_string(), 10);
        queue.push("Beta".to_string(), 20);

        let msgs = queue.active_messages();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].text(), "Alpha");
        assert_eq!(msgs[1].text(), "Beta");
    }

    #[test]
    fn empty_queue_returns_empty_slice() {
        let queue = HudMessageQueue::new(4);
        let msgs = queue.active_messages();
        assert!(msgs.is_empty());
        assert_eq!(msgs.len(), 0);
    }

    #[test]
    fn multiple_ticks_expire_message() {
        let mut queue = HudMessageQueue::new(4);
        queue.push("Countdown".to_string(), 3);

        queue.tick(); // 2 remaining
        assert_eq!(queue.len(), 1);
        queue.tick(); // 1 remaining
        assert_eq!(queue.len(), 1);
        queue.tick(); // 0 remaining → removed
        assert!(queue.is_empty());
    }

    #[test]
    fn tick_on_empty_queue_is_noop() {
        let mut queue = HudMessageQueue::new(4);
        queue.tick(); // Should not panic.
        assert!(queue.is_empty());
    }

    #[test]
    fn max_messages_of_one() {
        let mut queue = HudMessageQueue::new(1);
        queue.push("Only".to_string(), 50);
        assert_eq!(queue.len(), 1);

        queue.push("Replacement".to_string(), 50);
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.active_messages()[0].text(), "Replacement");
    }

    #[test]
    fn message_is_expired_at_zero() {
        let msg = HudMessage::new("Test".to_string(), 0);
        assert!(msg.is_expired());

        let msg2 = HudMessage::new("Test".to_string(), 1);
        assert!(!msg2.is_expired());
    }

    #[test]
    fn push_with_zero_duration_removed_on_first_tick() {
        let mut queue = HudMessageQueue::new(4);
        queue.push("Instant".to_string(), 0);
        // Before tick, the message exists (duration is 0 but not yet ticked).
        // Actually, remaining_tics is 0, so it's already "expired" but still in queue
        // until tick() runs the retain.
        assert_eq!(queue.len(), 1);

        queue.tick();
        // tick decrements (already 0, stays 0), retain removes it.
        assert!(queue.is_empty());
    }

    #[test]
    fn interleaved_push_and_tick() {
        let mut queue = HudMessageQueue::new(4);
        queue.push("A".to_string(), 2);
        queue.tick(); // A=1

        queue.push("B".to_string(), 3);
        queue.tick(); // A=0(removed), B=2

        assert_eq!(queue.len(), 1);
        assert_eq!(queue.active_messages()[0].text(), "B");

        queue.push("C".to_string(), 1);
        queue.tick(); // B=1, C=0(removed)

        assert_eq!(queue.len(), 1);
        assert_eq!(queue.active_messages()[0].text(), "B");
    }
}
