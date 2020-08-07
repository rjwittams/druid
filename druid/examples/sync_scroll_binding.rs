use druid::widget::prelude::*;
use druid::widget::{
    Binding, BindingExt, Flex, Label,
    LensBindingExt, Padding, Scope, Scroll, TextBox, WidgetBindingExt,
};
use druid::{AppLauncher, Data, Lens, LensExt, LocalizedString, Vec2, WidgetExt, WindowDesc};
use druid::piet::{Color, Text, TextLayout, TextLayoutBuilder, FontBuilder};
use std::marker::PhantomData;

#[derive(Data, Lens, Debug, Clone)]
struct OuterState {
    name: String,
    job: String,
}

impl OuterState {
    pub fn new(name: String, job: String) -> Self {
        OuterState { name, job }
    }
}

#[derive(Data, Lens, Debug, Clone)]
struct InnerState {
    text: String,
    font: String,
    offsets: Vec2,
}

impl InnerState {
    pub fn new(text: String) -> Self {
        InnerState {
            text,
            font: "Courier".into(),
            offsets: Default::default(),
        }
    }
}

pub fn main() {
    let window = WindowDesc::new(build_widget)
        .window_size(Size::new(700.0, 300.0)) // build_inner_widget)
        .title(LocalizedString::new("scroll-demo-window-title").with_placeholder("Scroll demo"));
    AppLauncher::with_window(window)
        .use_simple_logger()
        .launch(OuterState::new("Piet Mondrian".into(), "Artist".into()))
        //.launch(InnerState::new("bob".into()))
        .expect("launch failed");
}

#[derive(Lens)]
struct LensedWidget {
    font_name: String,
    text: String,
}

impl LensedWidget {
    pub fn new(font_name: String, text: String) -> Self {
        LensedWidget { font_name, text }
    }
}

impl Widget<String> for LensedWidget {
    fn event(&mut self, _ctx: &mut EventCtx, _event: &Event, _data: &mut String, _env: &Env) {}

    fn lifecycle(&mut self, _ctx: &mut LifeCycleCtx, _event: &LifeCycle, _data: &String, _env: &Env) {}

    fn update(&mut self, _ctx: &mut UpdateCtx, _old_data: &String, _data: &String, _env: &Env) {}

    fn layout(
        &mut self,
        _ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        _data: &String,
        _env: &Env,
    ) -> Size {
        bc.constrain(Size::new(300.0, 100.0))
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &String, _env: &Env) {
        let rect = ctx.region().to_rect();
        ctx.fill(rect, &Color::WHITE);

        let try_font = ctx.text().new_font_by_name(&self.font_name, 15.0).build();

        let (font, found) = if try_font.is_ok() {
            (try_font.unwrap(), true)
        } else {
            (
                ctx.text().new_font_by_name("Arial", 15.0).build().unwrap(),
                false,
            )
        };

        if let Ok(layout) = ctx
            .text()
            .new_text_layout(
                &font,
                &format!(
                    "Data: {} Field: {} Font: {} Found: {}",
                    data, self.text, self.font_name, found
                ),
                200.0,
            )
            .build()
        {
            let fill_color = Color::BLACK;
            if let Some(metric) = layout.line_metric(0) {
                ctx.draw_text(&layout, (0.0, metric.height), &fill_color);
            }
        }
    }
}

struct BindingScrollOffsets<S, T, L :Lens<S, Vec2> > {
    data_lens: L,
    phantom_s: PhantomData<S>,
    phantom_t: PhantomData<T>,
}

impl<S, T, L :Lens<S, Vec2>> BindingScrollOffsets<S, T, L> {
    pub fn new(data_lens: L) -> Self {
        BindingScrollOffsets {
            data_lens,
            phantom_s: Default::default(),
            phantom_t: Default::default(),
        }
    }
}

impl <S, T, L :Lens<S, Vec2>, W: Widget<T>> Binding<S, Scroll<T, W>> for BindingScrollOffsets<S, T, L> {
    type Change = (); // No point copying the offsets in here, they are cheap to get off the scroll

    fn apply_data_to_controlled(
        &self,
        data: &S,
        controlled: &mut Scroll<T, W>,
        ctx: &mut UpdateCtx,
    ) {
        self.data_lens.with(data, |offsets|{
            controlled.scroll_to( offsets.clone(), ctx.size())
        });
        ctx.request_paint();
    }

    fn append_change_required(
        &self,
        controlled: &Scroll<T, W>,
        data: &S,
        change: &mut Option<Self::Change>,
    ) {
        self.data_lens.with(data, |offsets| {
            if !controlled.offset().same(offsets) {
                *change = Some(())
            }
        });
    }

    fn apply_change_to_data(
        &self,
        controlled: &Scroll<T, W>,
        data: &mut S,
        _change: Self::Change,
        _ctx: &mut EventCtx,
    ) {
        self.data_lens.with_mut( data, |offsets| {
            *offsets = controlled.offset()
        });
    }
}

fn build_widget() -> impl Widget<OuterState> {
    let row = Flex::row()
        .with_child(TextBox::new().lens(OuterState::name))
        .with_child(TextBox::new().lens(OuterState::job));

    let scope =
        Scope::new(InnerState::new, // How to construct the inner state from its input
                   InnerState::text,      // How to extract the input back out of the inner state
                   build_inner_widget()  // Widgets operating on inner state
        ).lens(OuterState::job);

    row.with_child(scope)
}

fn build_inner_widget() -> impl Widget<InnerState> {
    let mut row = Flex::row();

    let lensed = LensedWidget::new("Arial".into(), "Stuff".into())
        .lens(InnerState::text)
        .binding(
            // Bindings are bi directional- A lens from Data->Prop,  Prop<-Widget.
            InnerState::font.bind(LensedWidget::font_name)
                // And combines bindings - they are syncing different props
                .and(InnerState::text.bind(LensedWidget::text)
                    .forward()), // choose one direction or both
        );

    row.add_child(
        Flex::column()
            .with_child(TextBox::new().lens(InnerState::font))
            .with_child(lensed),
    );


    let follower = Scroll::new(make_col(1))
        .lens(InnerState::text)
        // This is a different idea for how to do a one way binding.
        .binding(BindingScrollOffsets::new(InnerState::offsets.read_only() )); //
    row.add_child(follower);

    let leader = Scroll::new(make_col(0))
        .lens(InnerState::text)
        .binding(BindingScrollOffsets::new(InnerState::offsets));
    row.add_child(leader);

    row
}

fn make_col(i: i32) -> Flex<String> {
    let mut col = Flex::column();

    for j in 0..30 {
        if i == j {
            col.add_child(Padding::new(3.0, TextBox::new()));
        } else {
            col.add_child(Padding::new(
                3.0,
                Label::new(move |d: &String, _env: &_| format!("Label {}, {}, {}", i, j, d)),
            ));
        };
    }
    col
}
