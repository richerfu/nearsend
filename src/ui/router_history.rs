use gpui::{App, AppContext as _, Context, Entity, Global, SharedString, Window, WindowId};
use gpui_router::use_navigate;
use rustc_hash::FxHashMap;
use std::collections::VecDeque;

/// Router history entry
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryEntry {
    pub pathname: SharedString,
}

impl HistoryEntry {
    pub fn new(pathname: impl Into<SharedString>) -> Self {
        Self {
            pathname: pathname.into(),
        }
    }
}

/// Router history manager
/// Manages navigation history for back operations
pub struct RouterHistory {
    /// History stack (back stack)
    history: VecDeque<HistoryEntry>,
    /// Maximum history size
    max_size: usize,
    /// Current position in history
    current_index: usize,
}

impl RouterHistory {
    /// Creates a new RouterHistory with default settings
    pub fn new() -> Self {
        Self {
            history: VecDeque::new(),
            max_size: 50,
            current_index: 0,
        }
    }

    /// Pushes a new entry to history (called when navigating to a new page)
    pub fn push(&mut self, entry: HistoryEntry) {
        if self
            .history
            .back()
            .map(|last| last.pathname == entry.pathname)
            .unwrap_or(false)
        {
            self.current_index = self.history.len();
            return;
        }

        // Remove all entries after current position (forward history)
        while self.history.len() > self.current_index {
            self.history.pop_back();
        }

        // Add new entry
        self.history.push_back(entry);
        self.current_index = self.history.len();

        // Trim history if exceeds max size
        while self.history.len() > self.max_size {
            self.history.pop_front();
            self.current_index = self.current_index.saturating_sub(1);
        }
    }

    /// Goes back to previous entry
    /// Returns the entry to navigate to, or None if at root
    pub fn go_back(&mut self) -> Option<HistoryEntry> {
        if self.current_index > 1 {
            self.current_index -= 1;
            self.history.get(self.current_index - 1).cloned()
        } else {
            None
        }
    }

    /// Checks if can go back
    pub fn can_go_back(&self) -> bool {
        self.current_index > 1
    }

    /// Resets history to a single entry.
    pub fn reset(&mut self, entry: HistoryEntry) {
        self.history.clear();
        self.current_index = 0;
        self.push(entry);
    }
}

impl Default for RouterHistory {
    fn default() -> Self {
        Self::new()
    }
}

enum NavigationRequest {
    Push(SharedString),
    Back,
    Replace(SharedString),
}

/// One window's navigation history and pending navigation requests.
///
/// Events and asynchronous work can enqueue requests without borrowing a
/// window. Its routed root applies them after `Routes` initializes that
/// window's router state, using the public Navigator API.
pub struct NavigationState {
    window_id: WindowId,
    history: RouterHistory,
    pending: VecDeque<NavigationRequest>,
}

impl NavigationState {
    fn new(window_id: WindowId) -> Self {
        let mut history = RouterHistory::new();
        history.push(HistoryEntry::new(super::routes::HOME));
        Self {
            window_id,
            history,
            pending: VecDeque::new(),
        }
    }

    pub fn pathname(&self) -> &SharedString {
        &self.history.history[self.history.current_index - 1].pathname
    }

    pub fn can_go_back(&self) -> bool {
        self.history.can_go_back()
    }

    pub fn navigate_to(&mut self, pathname: &str, cx: &mut Context<Self>) {
        if self.pathname().as_ref() == pathname {
            return;
        }
        let pathname = SharedString::from(pathname.to_owned());
        self.history.push(HistoryEntry::new(pathname.clone()));
        self.pending.push_back(NavigationRequest::Push(pathname));
        cx.notify();
    }

    pub fn go_back(&mut self, cx: &mut Context<Self>) -> bool {
        if self.history.go_back().is_none() {
            return false;
        }
        self.pending.push_back(NavigationRequest::Back);
        cx.notify();
        true
    }

    pub fn go_back_or_navigate(&mut self, fallback: &str, cx: &mut Context<Self>) {
        if !self.go_back(cx) {
            self.navigate_to(fallback, cx);
        }
    }

    pub fn reset(&mut self, pathname: &str, cx: &mut Context<Self>) {
        let pathname = SharedString::from(pathname.to_owned());
        self.history.reset(HistoryEntry::new(pathname.clone()));
        self.pending.push_back(NavigationRequest::Replace(pathname));
        cx.notify();
    }

    pub fn apply_pending(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        assert_eq!(self.window_id, window.window_handle().window_id());
        while let Some(request) = self.pending.pop_front() {
            let mut navigator = use_navigate(window, cx);
            match request {
                NavigationRequest::Push(pathname) => navigator.push(pathname),
                NavigationRequest::Back => navigator.back(),
                NavigationRequest::Replace(pathname) => navigator.replace(pathname),
            }
        }
    }
}

/// Resolves a window to its navigation owner, including callbacks from pages
/// that do not retain the application root.
#[derive(Default)]
pub struct NavigationRegistry {
    windows: FxHashMap<WindowId, Entity<NavigationState>>,
}

impl Global for NavigationRegistry {}

impl NavigationRegistry {
    pub fn init(cx: &mut App) {
        cx.set_global(Self::default());
        cx.on_window_closed(|cx, id| {
            cx.global_mut::<Self>().windows.remove(&id);
        })
        .detach();
    }

    pub fn register(window: &Window, cx: &mut App) -> Entity<NavigationState> {
        let id = window.window_handle().window_id();
        let navigation = cx.new(|_| NavigationState::new(id));
        assert!(cx
            .global_mut::<Self>()
            .windows
            .insert(id, navigation.clone())
            .is_none());
        navigation
    }

    pub fn for_window(window: &Window, cx: &App) -> Entity<NavigationState> {
        cx.global::<Self>()
            .windows
            .get(&window.window_handle().window_id())
            .expect("window navigation must be registered before creating its pages")
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{div, IntoElement as _, Render, TestAppContext, WindowHandle};
    use gpui_router::{use_location, Route, Routes};
    use std::{cell::RefCell, rc::Rc};

    struct RoutedWindow {
        navigation: Entity<NavigationState>,
        observed: Rc<RefCell<SharedString>>,
    }

    impl Render for RoutedWindow {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui::IntoElement {
            let navigation = self.navigation.clone();
            let observed = self.observed.clone();
            Routes::new().child(Route::new().path("*").element(move |window, cx| {
                navigation.update(cx, |navigation, cx| navigation.apply_pending(window, cx));
                *observed.borrow_mut() = use_location(cx).pathname.clone();
                div().into_any_element()
            }))
        }
    }

    fn open_window(
        cx: &mut TestAppContext,
        initial_request: Option<&str>,
    ) -> (WindowHandle<RoutedWindow>, Rc<RefCell<SharedString>>) {
        let observed = Rc::new(RefCell::new(SharedString::default()));
        let snapshot = observed.clone();
        let handle = cx.add_window(|window, cx| {
            let navigation = NavigationRegistry::register(window, cx);
            if let Some(pathname) = initial_request {
                navigation.update(cx, |navigation, cx| navigation.navigate_to(pathname, cx));
            }
            RoutedWindow {
                navigation,
                observed: snapshot,
            }
        });
        (handle, observed)
    }

    fn navigation(
        cx: &TestAppContext,
        handle: WindowHandle<RoutedWindow>,
    ) -> Entity<NavigationState> {
        cx.read_window(&handle, |view, cx| view.read(cx).navigation.clone())
            .unwrap()
    }

    fn draw(cx: &mut TestAppContext, handle: WindowHandle<RoutedWindow>) {
        cx.update_window(handle.into(), |_, window, cx| {
            let _ = window.draw(cx);
        })
        .unwrap();
    }

    fn init(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_router::init(cx);
            NavigationRegistry::init(cx);
        });
    }

    #[gpui::test]
    fn queues_navigation_before_the_first_routes_render(cx: &mut TestAppContext) {
        init(cx);
        let (window, observed) = open_window(cx, Some(super::super::routes::SEND_FILES));
        draw(cx, window);
        assert_eq!(*observed.borrow(), super::super::routes::SEND_FILES);

        let navigation = navigation(cx, window);
        assert!(navigation.update(cx, |navigation, cx| navigation.go_back(cx)));
        draw(cx, window);
        assert_eq!(*observed.borrow(), super::super::routes::HOME);
        assert!(!navigation.read_with(cx, |navigation, _| navigation.can_go_back()));
    }

    #[gpui::test]
    fn background_navigation_targets_its_own_window(cx: &mut TestAppContext) {
        init(cx);
        let (first, first_path) = open_window(cx, None);
        let (second, second_path) = open_window(cx, None);
        draw(cx, first);
        draw(cx, second);
        let first_navigation = navigation(cx, first);
        let second_navigation = navigation(cx, second);

        first_navigation.update(cx, |navigation, cx| {
            navigation.navigate_to(super::super::routes::RECEIVE_HISTORY, cx);
        });
        second_navigation.update(cx, |navigation, cx| {
            navigation.navigate_to(super::super::routes::SETTINGS_ABOUT, cx);
        });
        draw(cx, first);
        draw(cx, second);
        assert_eq!(*first_path.borrow(), super::super::routes::RECEIVE_HISTORY);
        assert_eq!(*second_path.borrow(), super::super::routes::SETTINGS_ABOUT);

        first_navigation.update(cx, |navigation, cx| assert!(navigation.go_back(cx)));
        draw(cx, first);
        assert_eq!(*first_path.borrow(), super::super::routes::HOME);
        assert_eq!(*second_path.borrow(), super::super::routes::SETTINGS_ABOUT);
    }

    #[gpui::test]
    fn applies_queued_history_and_share_reset_in_order(cx: &mut TestAppContext) {
        init(cx);
        let (window, observed) = open_window(cx, None);
        let navigation = navigation(cx, window);
        navigation.update(cx, |navigation, cx| {
            navigation.navigate_to(super::super::routes::SEND_FILES, cx);
            navigation.navigate_to(super::super::routes::TRANSFER_PROGRESS, cx);
            assert!(navigation.go_back(cx));
        });
        draw(cx, window);
        assert_eq!(*observed.borrow(), super::super::routes::SEND_FILES);

        navigation.update(cx, |navigation, cx| {
            navigation.reset(super::super::routes::HOME, cx);
            navigation.navigate_to(super::super::routes::RECEIVE_INCOMING, cx);
        });
        draw(cx, window);
        assert_eq!(*observed.borrow(), super::super::routes::RECEIVE_INCOMING);
        navigation.update(cx, |navigation, cx| assert!(navigation.go_back(cx)));
        draw(cx, window);
        assert_eq!(*observed.borrow(), super::super::routes::HOME);
    }
}
