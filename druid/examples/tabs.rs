use druid::widget::{Button, CrossAxisAlignment, Flex, Label, Tabs};
use druid::{
    AppLauncher, Data, Env, Lens, Widget, WidgetExt, WindowDesc
};

#[derive(Data, Clone)]
struct Basic {}

#[derive(Data, Clone, Lens)]
struct Advanced {
    number: usize,
}

#[derive(Data, Clone, Lens)]
struct AppState {
    basic: Basic,
    advanced: Advanced,
}

pub fn main() {
    // describe the main window
    let main_window = WindowDesc::new(build_root_widget)
        .title("Tabs")
        .window_size((400.0, 400.0));

    // create the initial app state
    let initial_state = AppState {
        basic: Basic {},
        advanced: Advanced { number: 13 },
    };

    // start the application
    AppLauncher::with_window(main_window)
        .use_simple_logger()
        .launch(initial_state)
        .expect("Failed to launch application");
}

fn build_root_widget() -> impl Widget<AppState> {
    let adv = Flex::column()
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .with_child(Label::new("More involved!"))
        .with_child(
            Button::new("Increase")
                .on_click(|_c, d: &mut usize, _e| *d += 1)
                .lens(Advanced::number),
        )
        .with_child(Label::new(|adv: &Advanced, _e: &Env| {
            format!("My number is {}", adv.number)
        }))
        .lens(AppState::advanced);

    Tabs::new()
        .with_tab("Basic", Label::new("Basic kind of stuff"))
        .with_tab("Advanced", adv)
}
