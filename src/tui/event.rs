use std::time::Duration;

use crossterm::event::{Event as CrosstermEvent, EventStream, KeyEventKind};
use futures_lite::StreamExt;
use tokio::sync::mpsc;

/// Application-level events.
#[derive(Debug, Clone)]
pub enum Event {
    /// A key was pressed.
    Key(crossterm::event::KeyEvent),
    /// Terminal was resized (values used by ratatui's `frame.area()` implicitly).
    #[allow(dead_code)]
    Resize(u16, u16),
    /// Tick — time to poll for data updates.
    Tick,
    /// Render — time to redraw the UI.
    Render,
    /// The file watcher detected changes and updated the cache.
    DataChanged,
    /// A warning from the background watcher or data loading.
    /// Displayed briefly in the status bar instead of printing to stderr.
    Warning(String),
}

/// Drives the event loop, forwarding crossterm events and emitting periodic
/// tick / render events through an `mpsc` channel.
pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<Event>,
    tx: mpsc::UnboundedSender<Event>,
    tick_rate: Duration,
    render_rate: Duration,
}

impl EventHandler {
    /// Create a new event handler.
    ///
    /// * `tick_rate` — how often to emit `Event::Tick` (data poll interval).
    /// * `render_rate` — how often to emit `Event::Render` (frame rate).
    #[must_use]
    pub fn new(tick_rate: Duration, render_rate: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            rx,
            tx,
            tick_rate,
            render_rate,
        }
    }

    /// Get a clone of the sender for external use (e.g. file watcher, Phase 2).
    #[must_use]
    #[allow(dead_code)]
    pub fn sender(&self) -> mpsc::UnboundedSender<Event> {
        self.tx.clone()
    }

    /// Start the background event loop. This spawns a tokio task that
    /// reads crossterm events and emits tick/render events on intervals.
    pub fn start(&self) {
        let tx = self.tx.clone();
        let tick_rate = self.tick_rate;
        let render_rate = self.render_rate;

        tokio::spawn(async move {
            let mut crossterm_events = EventStream::new();
            let mut tick_interval = tokio::time::interval(tick_rate);
            let mut render_interval = tokio::time::interval(render_rate);

            tick_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            render_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                let event = tokio::select! {
                    // Crossterm terminal events (key presses, resize, etc.)
                    maybe_event = crossterm_events.next() => {
                        match maybe_event {
                            Some(Ok(evt)) => match evt {
                                CrosstermEvent::Key(key) if key.kind == KeyEventKind::Press => {
                                    Some(Event::Key(key))
                                }
                                CrosstermEvent::Resize(w, h) => Some(Event::Resize(w, h)),
                                _ => None,
                            },
                            // Stream ended or error — stop the loop
                            Some(Err(_)) | None => break,
                        }
                    }
                    // Periodic tick for data refresh
                    _ = tick_interval.tick() => {
                        Some(Event::Tick)
                    }
                    // Periodic render
                    _ = render_interval.tick() => {
                        Some(Event::Render)
                    }
                };

                if let Some(e) = event {
                    if tx.send(e).is_err() {
                        break;
                    }
                }
            }
        });
    }

    /// Receive the next event. Returns `None` if the channel is closed.
    pub async fn next(&mut self) -> Option<Event> {
        self.rx.recv().await
    }

    /// Try to receive the next event without blocking.
    #[allow(dead_code)]
    pub fn try_next(&mut self) -> Result<Event, mpsc::error::TryRecvError> {
        self.rx.try_recv()
    }
}
