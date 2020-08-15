// Copyright 2019 The Druid Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Window building and app lifecycle.

use crate::ext_event::{ExtEventHost, ExtEventSink};
use crate::kurbo::{Point, Size};
use crate::shell::{Application, Error as PlatformError, WindowBuilder, WindowHandle, WindowLevel};
use crate::widget::LabelText;
use crate::win_handler::{AppHandler, AppState};
use crate::window::WindowId;
use crate::{theme, AppDelegate, Data, Env, LocalizedString, MenuDesc, Widget, WidgetExt};

use druid_shell::WindowState;

/// A function that modifies the initial environment.
type EnvSetupFn<T> = dyn FnOnce(&mut Env, &T);

/// Handles initial setup of an application, and starts the runloop.
pub struct AppLauncher<T> {
    windows: Vec<WindowDesc<T>>,
    env_setup: Option<Box<EnvSetupFn<T>>>,
    delegate: Option<Box<dyn AppDelegate<T>>>,
    ext_event_host: ExtEventHost,
}

pub struct WindowConfig {
    pub(crate) size: Option<Size>,
    pub(crate) min_size: Option<Size>,
    pub(crate) position: Option<Point>,
    pub(crate) resizable: Option<bool>,
    pub(crate) show_titlebar: Option<bool>,
    pub(crate) maximized: Option<bool>,
    pub(crate) minimized: Option<bool>,
    pub(crate) level: Option<WindowLevel>,
    pub(crate) state: WindowState,
}

/// A description of a window to be instantiated.
///
/// This includes a function that can build the root widget, as well as other
/// window properties such as the title.
pub struct WindowDesc<T> {
    pub(crate) pending: PendingWindow<T>,
    pub(crate) config: WindowConfig,
    /// The `WindowId` that will be assigned to this window.
    ///
    /// This can be used to track a window from when it is launched and when
    /// it actually connects.
    pub id: WindowId,
}

pub struct PendingWindow<T> {
    pub(crate) root: Box<dyn Widget<T>>,
    pub(crate) title: LabelText<T>,
    pub(crate) menu: Option<MenuDesc<T>>,
}

impl<T: Data> PendingWindow<T> {
    pub fn new_from_boxed(root: Box<dyn Widget<T>>) -> PendingWindow<T> {
        PendingWindow {
            root,
            title: LocalizedString::new("app-name").into(),
            menu: MenuDesc::platform_default(),
        }
    }

    pub fn new<W, F>(root: F) -> PendingWindow<T>
    where
        W: Widget<T> + 'static,
        F: FnOnce() -> W + 'static,
    {
        PendingWindow {
            root: root().boxed(),
            title: LocalizedString::new("app-name").into(),
            menu: MenuDesc::platform_default(),
        }
    }

    /// Set the title for this window. This is a [`LabelText`]; it can be either
    /// a `String`, a [`LocalizedString`], or a closure that computes a string;
    /// it will be kept up to date as the application's state changes.
    ///
    /// [`LabelText`]: widget/enum.LocalizedString.html
    /// [`LocalizedString`]: struct.LocalizedString.html
    pub fn title(mut self, title: impl Into<LabelText<T>>) -> Self {
        self.title = title.into();
        self
    }

    /// Set the menu for this window.
    pub fn menu(mut self, menu: MenuDesc<T>) -> Self {
        self.menu = Some(menu);
        self
    }
}

impl<T: Data> AppLauncher<T> {
    /// Create a new `AppLauncher` with the provided window.
    pub fn with_window(window: WindowDesc<T>) -> Self {
        AppLauncher {
            windows: vec![window],
            env_setup: None,
            delegate: None,
            ext_event_host: ExtEventHost::new(),
        }
    }

    /// Provide an optional closure that will be given mutable access to
    /// the environment and immutable access to the app state before launch.
    ///
    /// This can be used to set or override theme values.
    pub fn configure_env(mut self, f: impl Fn(&mut Env, &T) + 'static) -> Self {
        self.env_setup = Some(Box::new(f));
        self
    }

    /// Set the [`AppDelegate`].
    ///
    /// [`AppDelegate`]: trait.AppDelegate.html
    pub fn delegate(mut self, delegate: impl AppDelegate<T> + 'static) -> Self {
        self.delegate = Some(Box::new(delegate));
        self
    }

    /// Initialize a minimal logger for printing logs out to stderr.
    ///
    /// Meant for use during development only.
    ///
    /// # Panics
    ///
    /// Panics if the logger fails to initialize.
    pub fn use_simple_logger(self) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        simple_logger::init().expect("Failed to init simple logger");
        #[cfg(target_arch = "wasm32")]
        console_log::init_with_level(log::Level::Trace).expect("Failed to init simple logger");
        self
    }

    /// Returns an [`ExtEventSink`] that can be moved between threads,
    /// and can be used to submit events back to the application.
    ///
    /// [`ExtEventSink`]: struct.ExtEventSink.html
    pub fn get_external_handle(&self) -> ExtEventSink {
        self.ext_event_host.make_sink()
    }

    /// Paint colorful rectangles for layout debugging.
    ///
    /// The rectangles are drawn around each widget's layout rect.
    #[deprecated(since = "0.5.0", note = "Use WidgetExt::debug_paint_layout instead.")]
    pub fn debug_paint_layout(self) -> Self {
        self
    }

    /// Build the windows and start the runloop.
    ///
    /// Returns an error if a window cannot be instantiated. This is usually
    /// a fatal error.
    pub fn launch(mut self, data: T) -> Result<(), PlatformError> {
        let app = Application::new()?;

        let mut env = theme::init();
        if let Some(f) = self.env_setup.take() {
            f(&mut env, &data);
        }

        let mut state = AppState::new(
            app.clone(),
            data,
            env,
            self.delegate.take(),
            self.ext_event_host,
        );

        for desc in self.windows {
            let window = desc.build_native(&mut state)?;
            window.show();
        }

        let handler = AppHandler::new(state);
        app.run(Some(Box::new(handler)));
        Ok(())
    }
}

impl Default for WindowConfig {
    fn default() -> Self {
        WindowConfig {
            size: None,
            min_size: None,
            position: None,
            resizable: None,
            show_titlebar: None,
            maximized: None,
            minimized: None,
            level: None,
        }
    }
}

impl WindowConfig {
    /// Set the window's initial drawing area size in [display points].
    ///
    /// You can pass in a tuple `(width, height)` or a [`Size`],
    /// e.g. to create a window with a drawing area 1000dp wide and 500dp high:
    ///
    /// ```ignore
    /// window.window_size((1000.0, 500.0));
    /// ```
    ///
    /// The actual window size in pixels will depend on the platform DPI settings.
    ///
    /// This should be considered a request to the platform to set the size of the window.
    /// The platform might increase the size a tiny bit due to DPI.
    ///
    /// [`Size`]: struct.Size.html
    /// [display points]: struct.Scale.html
    pub fn window_size(mut self, size: impl Into<Size>) -> Self {
        self.size = Some(size.into());
        self
    }

    /// Set the window's minimum drawing area size in [display points].
    ///
    /// The actual minimum window size in pixels will depend on the platform DPI settings.
    ///
    /// This should be considered a request to the platform to set the minimum size of the window.
    /// The platform might increase the size a tiny bit due to DPI.
    ///
    /// To set the window's initial drawing area size use [`window_size`].
    ///
    /// [`window_size`]: #method.window_size
    /// [display points]: struct.Scale.html
    pub fn with_min_size(mut self, size: impl Into<Size>) -> Self {
        self.min_size = Some(size.into());
        self
    }

    /// Set whether the window should be resizable.
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = Some(resizable);
        self
    }

    /// Set whether the window should have a titlebar and decorations.
    pub fn show_titlebar(mut self, show_titlebar: bool) -> Self {
        self.show_titlebar = Some(show_titlebar);
        self
    }

    /// Sets the initial window position in virtual screen coordinates.
    /// [`position`] Position in pixels.
    ///
    /// [`position`]: struct.Point.html
    pub fn set_position(mut self, position: Point) -> Self {
        self.position = Some(position);
        self
    }

    pub fn set_level(mut self, level: WindowLevel) -> Self {
        self.level = Some(level);
        self
    }

    /// Creates the window maximized.
    pub fn maximized(mut self) -> Self {
        self.maximized = Some(true);
        self
    }

    /// Creates the window minimized.
    pub fn minimized(mut self) -> Self {
        self.minimized = Some(true);
        self
    }

    pub fn apply_to_builder(&self, builder: &mut WindowBuilder) {
        if let Some(resizable) = self.resizable {
            builder.resizable(resizable);
        }

        if let Some(show_titlebar) = self.show_titlebar {
            builder.show_titlebar(show_titlebar);
        }

        if let Some(size) = self.size {
            builder.set_size(size);
        }
        if let Some(min_size) = self.min_size {
            builder.set_min_size(min_size);
        }

        if let Some(position) = self.position {
            builder.set_position(position);
        }

        if let Some(true) = self.maximized {
            builder.maximized();
        }

        if let Some(true) = self.minimized {
            builder.minimized();
        }

        if let Some(level) = self.level {
            log::info!("Set level on builder{:?}", level);
            builder.set_level(level)
        }
    }

    pub fn apply_to_handle(&self, win_handle: &mut WindowHandle) {
        if let Some(resizable) = self.resizable {
            win_handle.resizable(resizable);
        }

        if let Some(show_titlebar) = self.show_titlebar {
            win_handle.show_titlebar(show_titlebar);
        }

        if let Some(size) = self.size {
            win_handle.set_size(size);
        }
        if let Some(_min_size) = self.min_size {
            // win_handle.set_min_size(min_size);
        }

        if let Some(position) = self.position {
            win_handle.set_position(position);
        }

        if let Some(true) = self.maximized {
            win_handle.maximize();
        }

        if let Some(true) = self.minimized {
            win_handle.minimize();
        }

        if let Some(level) = self.level {
            win_handle.set_level(level)
        }
    }
}

impl<T: Data> WindowDesc<T> {
    /// Create a new `WindowDesc`, taking a function that will generate the root
    /// [`Widget`] for this window.
    ///
    /// It is possible that a `WindowDesc` can be reused to launch multiple windows.
    ///
    /// [`Widget`]: trait.Widget.html
    pub fn new<W, F>(root: F) -> WindowDesc<T>
    where
        W: Widget<T> + 'static,
        F: FnOnce() -> W + 'static,
    {
        // wrap this closure in another closure that boxes the created widget.
        // this just makes our API slightly cleaner; callers don't need to explicitly box.
        WindowDesc {
            pending: PendingWindow::new(root),
            config: WindowConfig::default(),
            id: WindowId::next(),
        }
    }

    /// Set the title for this window. This is a [`LabelText`]; it can be either
    /// a `String`, a [`LocalizedString`], or a closure that computes a string;
    /// it will be kept up to date as the application's state changes.
    ///
    /// [`LabelText`]: widget/enum.LocalizedString.html
    /// [`LocalizedString`]: struct.LocalizedString.html
    pub fn title(mut self, title: impl Into<LabelText<T>>) -> Self {
        self.pending = self.pending.title(title);
        self
    }

    /// Set the menu for this window.
    pub fn menu(mut self, menu: MenuDesc<T>) -> Self {
        self.pending = self.pending.menu(menu);
        self
    }

    /// Set the window's initial drawing area size in [display points].
    ///
    /// You can pass in a tuple `(width, height)` or a [`Size`],
    /// e.g. to create a window with a drawing area 1000dp wide and 500dp high:
    ///
    /// ```ignore
    /// window.window_size((1000.0, 500.0));
    /// ```
    ///
    /// The actual window size in pixels will depend on the platform DPI settings.
    ///
    /// This should be considered a request to the platform to set the size of the window.
    /// The platform might increase the size a tiny bit due to DPI.
    ///
    /// [`Size`]: struct.Size.html
    /// [display points]: struct.Scale.html
    pub fn window_size(mut self, size: impl Into<Size>) -> Self {
        self.config.size = Some(size.into());
        self
    }

    /// Set the window's minimum drawing area size in [display points].
    ///
    /// The actual minimum window size in pixels will depend on the platform DPI settings.
    ///
    /// This should be considered a request to the platform to set the minimum size of the window.
    /// The platform might increase the size a tiny bit due to DPI.
    ///
    /// To set the window's initial drawing area size use [`window_size`].
    ///
    /// [`window_size`]: #method.window_size
    /// [display points]: struct.Scale.html
    pub fn with_min_size(mut self, size: impl Into<Size>) -> Self {
        self.config = self.config.with_min_size(size);
        self
    }

    /// Set whether the window should be resizable.
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.config = self.config.resizable(resizable);
        self
    }

    /// Set whether the window should have a titlebar and decorations.
    pub fn show_titlebar(mut self, show_titlebar: bool) -> Self {
        self.config = self.config.show_titlebar(show_titlebar);
        self
    }

    /// Sets the initial window position in virtual screen coordinates.
    /// [`position`] Position in pixels.
    ///
    /// [`position`]: struct.Point.html
    pub fn set_position(mut self, position: Point) -> Self {
        self.config = self.config.set_position(position);
        self
    }

    /// Set initial state for the window.
    pub fn set_window_state(mut self, state: WindowState) -> Self {
        self.config = self.config.set_window_state(state);
        self
    }

    /// Attempt to create a platform window from this `WindowDesc`.
    pub(crate) fn build_native(
        self,
        state: &mut AppState<T>,
    ) -> Result<WindowHandle, PlatformError> {
        state.build_native_window(self.id, self.pending, self.config)
    }
}
