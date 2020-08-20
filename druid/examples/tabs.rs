use druid::widget::{
    Axis, Button, CrossAxisAlignment, Flex, Label, MainAxisAlignment, Padding, RadioGroup,
    SizedBox, TabOrientation, Tabs, ViewSwitcher,
};
use druid::{theme, AppLauncher, Color, Data, Env, Lens, LensExt, Widget, WidgetExt, WindowDesc};

#[derive(Data, Clone)]
struct Basic {}

#[derive(Data, Clone, Lens)]
struct Advanced {
    number: usize,
}

#[derive(Data, Clone, Lens)]
struct TabConfig {
    axis: Axis,
    cross: CrossAxisAlignment,
    rotation: TabOrientation,
}

#[derive(Data, Clone, Lens)]
struct AppState {
    tab_config: TabConfig,
    basic: Basic,
    advanced: Advanced,
}

pub fn main() {
    // describe the main window
    let main_window = WindowDesc::new(build_root_widget)
        .title("Tabs")
        .window_size((700.0, 400.0));

    // create the initial app state
    let initial_state = AppState {
        tab_config: TabConfig {
            axis: Axis::Horizontal,
            cross: CrossAxisAlignment::Start,
            rotation: TabOrientation::Standard,
        },
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
    fn decor<T: Data>(label: Label<T>) -> SizedBox<T> {
        label
            .padding(5.)
            .background(theme::PLACEHOLDER_COLOR)
            .expand_width()
    }

    fn group<T: Data, W: Widget<T> + 'static>(w: W) -> Padding<T> {
        w.border(Color::WHITE, 0.5).padding(5.)
    }

    let axis_picker = Flex::column()
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .with_child(decor(Label::new("Tab bar axis")))
        .with_child(RadioGroup::new(vec![
            ("Horizontal", Axis::Horizontal),
            ("Vertical", Axis::Vertical),
        ]))
        .lens(AppState::tab_config.then(TabConfig::axis));

    let cross_picker = Flex::column()
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .with_child(decor(Label::new("Tab bar alignment")))
        .with_child(RadioGroup::new(vec![
            ("Start", CrossAxisAlignment::Start),
            ("End", CrossAxisAlignment::End),
        ]))
        .lens(AppState::tab_config.then(TabConfig::cross));

    let rot_picker = Flex::column()
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .with_child(decor(Label::new("Tab rotation")))
        .with_child(RadioGroup::new(vec![
            ("Standard", TabOrientation::Standard),
            ("None", TabOrientation::Turns(0)),
            ("Up", TabOrientation::Turns(3)),
            ("Down", TabOrientation::Turns(1)),
            ("Aussie", TabOrientation::Turns(2)),
        ]))
        .lens(AppState::tab_config.then(TabConfig::rotation));

    let sidebar = Flex::column()
        .main_axis_alignment(MainAxisAlignment::Start)
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .with_child(group(axis_picker))
        .with_child(group(cross_picker))
        .with_child(group(rot_picker))
        .with_flex_spacer(1.)
        .fix_width(200.0);

    let vs = ViewSwitcher::new(
        |app_s: &AppState, _| app_s.tab_config.clone(),
        |tc: &TabConfig, _, _| Box::new(build_tab_widget(tc)),
    );
    Flex::row().with_child(sidebar).with_flex_child(vs, 1.0)
}

fn build_tab_widget(tab_config: &TabConfig) -> impl Widget<AppState> {
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
        .with_axis(tab_config.axis)
        .with_cross_axis_alignment(tab_config.cross)
        .with_rotation(tab_config.rotation)
        .with_tab("Basic", Label::new("Basic kind of stuff"))
        .with_tab("Advanced", adv)
        .with_tab("Page 3", Label::new("Basic kind of stuff"))
        .with_tab("Page 4", Label::new("Basic kind of stuff"))
        .with_tab("Page 5", Label::new("Basic kind of stuff"))
        .with_tab("Page 6", Label::new("Basic kind of stuff"))
        .with_tab("Page 7", Label::new("Basic kind of stuff"))
}
