//! The OS event loop owns launcher, game, save-before-close and service lifetimes.
//! Every GPU owner retires with its context current, before its native window.

use std::{
    cell::RefCell,
    future::Future,
    pin::Pin,
    rc::Rc,
    time::{Duration, Instant},
};

use solomon_web::{
    Game,
    desktop::{
        DesktopControls,
        launcher::{Action, Build, DesktopState, Launcher, SessionOptions, Update},
    },
    platform::{ClientTasks, Surface},
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    window::WindowId,
};

use crate::{
    config::{self, Config},
    profile::{OpenProfile, Profile},
    session::Session,
    updates::{self, AvailableUpdate},
    window::NativeWindow,
};

#[cfg(target_os = "macos")]
mod macos;

type AccessibilityEvent = egui_winit::accesskit_winit::Event;
type Pending<T> = Pin<Box<dyn Future<Output = T>>>;

enum Screen {
    Launcher { view: Box<Launcher>, window: NativeWindow },
    Game { game: Rc<RefCell<Game>>, session: Box<Session>, window: NativeWindow },
}

impl Screen {
    fn window(&self) -> &NativeWindow {
        match self {
            Self::Launcher { window, .. } | Self::Game { window, .. } => window,
        }
    }
    fn focus(&self) {
        self.window().window.set_visible(true);
        self.window().window.focus_window();
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        if let Err(error) = self.window().make_current() {
            solomon_web::platform::error(&error);
        }
    }
}

struct PreparedGame {
    game: Rc<RefCell<Game>>,
    session: Session,
}

struct Loading {
    future: Pending<Result<PreparedGame, String>>,
    window: Option<NativeWindow>,
}
impl Loading {
    fn window(&self) -> &NativeWindow {
        self.window.as_ref().expect("a pending load owns its window")
    }
    fn take_window(&mut self) -> NativeWindow {
        self.window.take().expect("a completed load owns its window")
    }
}
impl Drop for Loading {
    fn drop(&mut self) {
        if let Some(window) = &self.window
            && let Err(error) = window.make_current()
        {
            solomon_web::platform::error(&error);
        }
    }
}

#[derive(Clone)]
enum Destination {
    Quit,
    Launcher,
    Reload(SessionOptions),
}
enum Closing {
    Saving { destination: Destination, deadline: Instant },
    Failed { destination: Destination },
}

struct App {
    // Dropping pending work first releases its Game/GPU references.
    tasks: Option<ClientTasks>,
    loading: Option<Loading>,
    screen: Option<Screen>,
    controls: DesktopControls,
    closing: Option<Closing>,
    failure: Option<String>,
    update: Option<AvailableUpdate>,
    checking: Option<Pending<Result<Option<AvailableUpdate>, String>>>,
    manual_check: bool,
    next_check: Instant,
    next_redraw: Instant,
    proxy: EventLoopProxy<AccessibilityEvent>,
    config: Config,
    profile: Profile,
    finished: bool,
    fatal: Option<String>,
}

pub fn run(config: Config) -> Result<(), String> {
    let OpenProfile::Primary(profile) = Profile::open(&config.profile)? else { return Ok(()) };
    solomon_graphics::assets::configure(solomon_graphics::assets::NativeAssets::new(
        &config.assets,
    )?);
    solomon_web::configure_native_storage(profile.documents.clone())?;
    let event_loop = EventLoop::<AccessibilityEvent>::with_user_event()
        .build()
        .map_err(|error| error.to_string())?;
    let mut addresses: Vec<String> = if_addrs::get_if_addrs()
        .unwrap_or_else(|error| {
            solomon_web::platform::error(&format!("Cannot list local network addresses: {error}"));
            Vec::new()
        })
        .into_iter()
        .filter(|address| !address.is_loopback())
        .filter_map(|address| match address.ip() {
            std::net::IpAddr::V4(address) => Some(address.to_string()),
            _ => None,
        })
        .collect();
    addresses.sort();
    addresses.dedup();
    let controls = DesktopControls::install(DesktopState {
        build: Build { revision: config.build.revision.clone() },
        addresses,
        update: None,
        update_error: None,
    });
    let mut app = App {
        tasks: Some(ClientTasks::install()),
        loading: None,
        screen: None,
        controls,
        closing: None,
        failure: None,
        update: None,
        checking: None,
        manual_check: false,
        next_check: Instant::now(),
        next_redraw: Instant::now(),
        proxy: event_loop.create_proxy(),
        config,
        profile,
        finished: false,
        fatal: None,
    };
    event_loop.run_app(&mut app).map_err(|error| error.to_string())?;
    app.retire_screen();
    app.fatal.map_or(Ok(()), Err)
}

impl App {
    fn open_launcher(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let window = NativeWindow::open(event_loop, "Solomon Darker", [1360, 850])?;
        window.window.set_min_inner_size(Some(winit::dpi::LogicalSize::new(960, 600)));
        // SAFETY: Screen retires the launcher and all GPU caches before this window.
        let renderer = unsafe { window.renderer()? };
        let mut view = futures_lite::future::block_on(Launcher::load(
            Surface::from_window(window.window.clone()),
            renderer,
            self.controls.state(),
        ))?;
        view.show_previous_app_data(self.profile.has_previous_app_data()?);
        view.native_ui().init_accesskit(event_loop, self.proxy.clone());
        view.native_ui().context().enable_accesskit();
        window.window.set_visible(true);
        window.window.request_redraw();
        self.screen = Some(Screen::Launcher { view: Box::new(view), window });
        Ok(())
    }

    fn start_session(
        &mut self,
        event_loop: &ActiveEventLoop,
        options: SessionOptions,
    ) -> Result<(), String> {
        if self.loading.is_some() || !matches!(self.screen, Some(Screen::Launcher { .. })) {
            return Err("Return to the launcher first.".into());
        }
        self.controls.set_message(None);
        let window = NativeWindow::open(event_loop, "Solomon Darker", [1360, 850])?;
        window.window.set_min_inner_size(Some(winit::dpi::LogicalSize::new(960, 600)));
        // SAFETY: Loading and then Screen retain this window until all game owners drop.
        let renderer = unsafe { window.renderer()? };
        let surface = Surface::from_window(window.window.clone());
        let controls = self.controls.clone();
        let assets = self.config.assets.clone();
        let revision = self.config.build.revision.clone();
        let future = Box::pin(async move {
            let session =
                blocking::unblock(move || Session::start(options, assets, &revision)).await?;
            let endpoint = session.endpoint();
            let location = endpoint.url.replacen("ws:", "http:", 1);
            solomon_web::platform::configure_native_location(&location, None)?;
            let game = Game::load_native(surface, renderer, endpoint, controls).await?;
            Ok(PreparedGame { game, session })
        });
        self.loading = Some(Loading { future, window: Some(window) });
        Ok(())
    }

    fn poll_loading(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let Some(loading) = &mut self.loading else { return Ok(()) };
        loading.window().make_current()?;
        let Some(result) = poll(&mut loading.future) else { return Ok(()) };
        let mut loading = self.loading.take().expect("a completed load has a window");
        match result {
            Ok(PreparedGame { game, session }) => {
                game.borrow_mut().native_ui().init_accesskit(event_loop, self.proxy.clone());
                game.borrow_mut().native_ui().context().enable_accesskit();
                self.controls.set_invite(session.invite.clone());
                // Retire the previous launcher with its own context current.
                self.screen.take();
                loading.window().make_current()?;
                loading.window().window.set_visible(true);
                loading.window().window.request_redraw();
                // Moving a field from a Drop owner would bypass its context ordering.
                // The finished future holds no GPU owners; swap it into a plain owner.
                let window = loading.take_window();
                self.screen = Some(Screen::Game { game, session: Box::new(session), window });
            }
            Err(error) => {
                drop(loading);
                solomon_web::platform::error(&error);
                self.message(error);
            }
        }
        Ok(())
    }

    fn message(&mut self, message: String) {
        if let Some(Screen::Launcher { view, .. }) = &mut self.screen {
            view.failed(message);
        } else {
            self.controls.set_message(Some(message));
        }
    }

    fn retire_screen(&mut self) {
        // Cancellation can release GPU owners captured by an unfinished client task.
        let window = self
            .loading
            .as_ref()
            .map(Loading::window)
            .or_else(|| self.screen.as_ref().map(Screen::window));
        if let Some(window) = window
            && let Err(error) = window.make_current()
        {
            solomon_web::platform::error(&error);
        }
        self.tasks.take();
        self.loading.take();
        self.screen.take();
        self.controls.set_close_error(None);
        self.controls.set_invite(None);
        self.controls.set_message(None);
        self.failure = None;
    }

    fn complete_transition(
        &mut self,
        event_loop: &ActiveEventLoop,
        destination: Destination,
    ) -> Result<(), String> {
        self.closing = None;
        self.retire_screen();
        match destination {
            Destination::Quit => {
                self.finished = true;
                event_loop.exit();
                #[cfg(target_os = "macos")]
                macos::reply(true);
            }
            Destination::Launcher => {
                self.tasks = Some(ClientTasks::install());
                self.open_launcher(event_loop)?;
            }
            Destination::Reload(options) => {
                self.tasks = Some(ClientTasks::install());
                self.open_launcher(event_loop)?;
                self.start_session(event_loop, options)?;
            }
        }
        Ok(())
    }

    fn request_transition(
        &mut self,
        event_loop: &ActiveEventLoop,
        destination: Destination,
    ) -> Result<(), String> {
        if let Some(closing) = &mut self.closing {
            if matches!(destination, Destination::Quit) {
                let (Closing::Saving { destination, .. } | Closing::Failed { destination }) =
                    closing;
                *destination = Destination::Quit;
            }
            return Ok(());
        }
        let saving = match &self.screen {
            Some(Screen::Game { game, .. }) => game.borrow_mut().begin_desktop_close(),
            _ => Ok(false),
        };
        match saving {
            Ok(false) => self.complete_transition(event_loop, destination)?,
            Ok(true) => {
                self.closing = Some(Closing::Saving {
                    destination,
                    deadline: Instant::now() + Duration::from_secs(20),
                })
            }
            Err(error) => self.close_failed(destination, error)?,
        }
        Ok(())
    }

    fn close_failed(&mut self, destination: Destination, error: String) -> Result<(), String> {
        if let Some(Screen::Game { game, .. }) = &self.screen {
            game.borrow_mut().cancel_desktop_close(&error)?;
        }
        self.controls.set_close_error(Some(error));
        self.closing = Some(Closing::Failed { destination });
        if let Some(screen) = &self.screen {
            screen.focus();
        }
        Ok(())
    }

    fn poll_close(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let Some(Closing::Saving { destination, deadline }) = &self.closing else { return Ok(()) };
        let destination = destination.clone();
        let result = if Instant::now() >= *deadline {
            Err("The game did not finish saving.".into())
        } else if let Some(Screen::Game { game, .. }) = &self.screen {
            let result =
                game.borrow_mut().poll_desktop_close(solomon_web::platform::monotonic_ms());
            Game::run_pending_work(game);
            result
        } else {
            Err("The game is no longer available for saving.".into())
        };
        match result {
            Ok(true) => self.complete_transition(event_loop, destination)?,
            Ok(false) => {}
            Err(error) => self.close_failed(destination, error)?,
        }
        Ok(())
    }

    fn check_updates(&mut self, manual: bool) {
        self.manual_check |= manual;
        if self.checking.is_some() {
            return;
        }
        let build = self.config.build.clone();
        self.checking = Some(Box::pin(blocking::unblock(move || {
            updates::check(&build, config::platform(), config::architecture())
        })));
        self.next_check = Instant::now() + Duration::from_secs(4 * 60 * 60);
    }

    fn poll_updates(&mut self) {
        if self.checking.is_none() && Instant::now() >= self.next_check {
            self.check_updates(false);
        }
        let Some(checking) = &mut self.checking else { return };
        let Some(result) = poll(checking) else { return };
        self.checking = None;
        let mut state = self.controls.state();
        match result {
            Ok(update) => {
                self.update = update;
                state.update_error = None;
            }
            Err(_) => {
                state.update_error =
                    Some("Updates could not be checked. You can keep playing offline.".into())
            }
        }
        state.update =
            self.update.as_ref().map(|update| Update { revision: update.revision.clone() });
        self.controls.receive(state.clone());
        if let Some(Screen::Launcher { view, .. }) = &mut self.screen {
            if self.manual_check {
                view.update_checked(state.clone());
            } else {
                view.receive(state.clone());
            }
        }
        if self.manual_check && matches!(self.screen, Some(Screen::Game { .. })) {
            self.message(state.update_error.unwrap_or_else(|| {
                if state.update.is_some() {
                    "An update is ready to download."
                } else {
                    "You have the latest available build."
                }
                .into()
            }));
        }
        self.manual_check = false;
    }

    fn action(&mut self, event_loop: &ActiveEventLoop, action: Action) -> Result<(), String> {
        match action {
            Action::Start(options) => self.start_session(event_loop, options),
            Action::Launcher => self.request_transition(event_loop, Destination::Launcher),
            Action::ReloadGame => {
                let Some(Screen::Game { session, .. }) = &self.screen else { return Ok(()) };
                self.request_transition(event_loop, Destination::Reload(session.options.clone()))
            }
            Action::Check => {
                self.check_updates(true);
                Ok(())
            }
            Action::Download => {
                open_url(&self.update.as_ref().ok_or("No update is available.")?.url)
            }
            Action::Website => open_url("https://solomondarker.com"),
            Action::OpenUrl(url) => open_url(&url),
            Action::CopyInvite => {
                let invite =
                    self.controls.invite().ok_or("Start hosting a game to create an invite.")?;
                let copied = solomon_web::platform::copy_text(&invite)?;
                let controls = self.controls.clone();
                solomon_web::platform::spawn_local(async move {
                    controls.set_message(Some(match copied.await {
                        Ok(()) => "Multiplayer invite copied.".into(),
                        Err(error) => error,
                    }));
                });
                Ok(())
            }
            Action::KeepPlaying => {
                if matches!(self.closing, Some(Closing::Failed { .. })) {
                    self.closing = None;
                    #[cfg(target_os = "macos")]
                    macos::reply(false);
                    self.controls.set_close_error(None);
                }
                Ok(())
            }
            Action::CloseWithoutSaving => {
                if let Some(Closing::Failed { destination }) = self.closing.take() {
                    self.complete_transition(event_loop, destination)?;
                }
                Ok(())
            }
        }
    }

    fn tick(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        if macos::requested() {
            self.request_transition(event_loop, Destination::Quit)?;
        }
        if self.finished {
            return Ok(());
        }
        if self.profile.focus_requested()?
            && let Some(screen) = &self.screen
        {
            screen.focus();
        }
        // Client tasks may upload or retire textures; keep their current owner active.
        if let Some(loading) = &self.loading {
            loading.window().make_current()?;
        } else if let Some(screen) = &self.screen {
            screen.window().make_current()?;
        }
        if let Some(tasks) = &self.tasks {
            tasks.tick();
        }
        self.poll_loading(event_loop)?;
        self.poll_updates();
        self.poll_close(event_loop)?;
        for action in self.controls.take_actions() {
            if self.finished {
                break;
            }
            if let Err(error) = self.action(event_loop, action) {
                self.message(error);
            }
        }
        if let Some(Screen::Game { session, .. }) = &self.screen
            && let Some(error) = session.failure()
        {
            self.controls.set_message(Some(error.into()));
        }
        if Instant::now() >= self.next_redraw
            && let Some(screen) = &self.screen
        {
            screen.window().window.request_redraw();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_redraw));
        Ok(())
    }

    fn fatal(&mut self, event_loop: &ActiveEventLoop, error: String) {
        self.fatal = Some(error);
        self.retire_screen();
        self.finished = true;
        event_loop.exit();
    }
}

impl ApplicationHandler<AccessibilityEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.screen.is_some() || self.finished {
            return;
        }
        #[cfg(target_os = "macos")]
        if let Err(error) = macos::install() {
            self.fatal(event_loop, error);
            return;
        }
        if let Err(error) = self.open_launcher(event_loop) {
            self.fatal(event_loop, error);
        }
    }

    fn user_event(&mut self, _: &ActiveEventLoop, event: AccessibilityEvent) {
        let Some(screen) = &mut self.screen else { return };
        if screen.window().window.id() != event.window_id {
            return;
        }
        let now = solomon_web::platform::monotonic_ms();
        let result = match event.window_event {
            egui_winit::accesskit_winit::WindowEvent::InitialTreeRequested => {
                match screen {
                    Screen::Launcher { view, .. } => view.native_ui().context().enable_accesskit(),
                    Screen::Game { game, .. } => {
                        game.borrow_mut().native_ui().context().enable_accesskit()
                    }
                }
                Ok(())
            }
            egui_winit::accesskit_winit::WindowEvent::ActionRequested(action) => match screen {
                Screen::Launcher { view, .. } => view.accesskit_action(action, now),
                Screen::Game { game, .. } => {
                    if self.failure.is_some() {
                        game.borrow_mut().native_ui().accesskit_action(action)
                    } else {
                        game.borrow_mut().accesskit_action(action, now)
                    }
                }
            },
            egui_winit::accesskit_winit::WindowEvent::AccessibilityDeactivated => Ok(()),
        };
        if let Err(error) = result {
            self.message(error);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.finished
            || self.screen.as_ref().is_none_or(|screen| screen.window().window.id() != window_id)
        {
            return;
        }
        if matches!(event, WindowEvent::CloseRequested) {
            if let Err(error) = self.request_transition(event_loop, Destination::Quit) {
                self.fatal(event_loop, error);
            }
            return;
        }
        let result = (|| {
            let now = solomon_web::platform::monotonic_ms();
            let screen = self.screen.as_mut().expect("the event belongs to the current screen");
            screen.window().make_current()?;
            if let WindowEvent::Resized(size) = &event {
                screen.window().resize(size.width, size.height);
            }
            match screen {
                Screen::Launcher { view, window } => {
                    view.window_event(&event, now)?;
                    if matches!(event, WindowEvent::RedrawRequested) {
                        view.frame(now)?;
                        self.next_redraw = window.present()?;
                    }
                    if let Some(action) = view.take_action() {
                        self.controls.request(action);
                    }
                }
                Screen::Game { game, window, .. } => {
                    // A failed game accepts input only through its recovery surface.
                    if self.failure.is_some() {
                        game.borrow_mut().native_ui().window_event(&event)?;
                    } else {
                        game.borrow_mut().window_event(&event, now)?;
                    }
                    if matches!(event, WindowEvent::RedrawRequested) {
                        if self.failure.is_none() {
                            let result = game.borrow_mut().frame(now);
                            if let Err(error) = result {
                                solomon_web::platform::error(&error);
                                if let Err(stop_error) = game.borrow_mut().report_failure(&error) {
                                    solomon_web::platform::error(&stop_error);
                                }
                                self.failure = Some(error);
                            }
                        }
                        if self.failure.is_some() {
                            game.borrow_mut().present_failure(now)?;
                        }
                        Game::run_pending_work(game);
                        self.next_redraw = window.present()?;
                    }
                }
            }
            Ok::<_, String>(())
        })();
        if let Err(error) = result {
            self.message(error);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.finished {
            return;
        }
        if let Err(error) = self.tick(event_loop) {
            self.fatal(event_loop, error);
        }
    }
}

fn poll<T>(future: &mut Pending<T>) -> Option<T> {
    futures_lite::future::block_on(futures_lite::future::poll_once(future.as_mut()))
}

fn open_url(value: &str) -> Result<(), String> {
    let url = url::Url::parse(value).map_err(|_| "The external link is invalid.")?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return Err("The external link must be an HTTPS address.".into());
    }
    webbrowser::open(url.as_str()).map_err(|error| format!("Could not open the browser: {error}"))
}
