use druid::widget::prelude::*;
use druid::widget::{Flex, Label, Padding, Scroll, Scope, TextBox, Binding, BindingHost, LensBinding, DataToWidgetOnlyBinding};
use druid::{AppLauncher, LocalizedString, WidgetExt, WindowDesc, Vec2, Data, Lens};
use std::marker::PhantomData;
use druid_shell::IntoKey;
use druid_shell::piet::{Text, TextLayoutBuilder, TextLayout, PietFont, Color};
use piet_common::FontBuilder;

#[derive(Data, Lens, Debug, Clone)]
struct OuterState{
    name: String,
    job: String
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
    offsets: Vec2
}

impl InnerState {
    pub fn new(text: String) -> Self {
        InnerState { text, font: "Courier".into(),  offsets: Default::default() }
    }
}

pub fn main() {
    let window = WindowDesc::new(build_widget).window_size( Size::new(700.0, 300.0) )// build_inner_widget)
        .title(LocalizedString::new("scroll-demo-window-title").with_placeholder("Scroll demo"));
    AppLauncher::with_window(window)
        .use_simple_logger()
        .launch(OuterState::new("Piet Mondrian".into(), "Artist".into() ))
        //.launch(InnerState::new("bob".into()))
        .expect("launch failed");
}

#[derive(Lens)]
struct LensedWidget{
    font_name: String,
    text: String
}

impl LensedWidget {
    pub fn new(font_name: String, text: String) -> Self {
        LensedWidget { font_name, text }
    }
}


impl Widget<String> for LensedWidget{
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut String, env: &Env) {

    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &String, env: &Env) {

    }

    fn update(&mut self, ctx: &mut UpdateCtx, old_data: &String, data: &String, env: &Env) {

    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &String, env: &Env) -> Size {
        bc.constrain( Size::new(300.0, 100.0) )
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &String, env: &Env) {
        let rect = ctx.region().to_rect();
        ctx.fill(rect, &Color::WHITE );

        let try_font = ctx
            .text()
            .new_font_by_name(&self.font_name, 15.0)
            .build();


        let (font, found) =  if try_font.is_ok(){
            (try_font.unwrap(), true)
        }else{
            (ctx
                .text()
                .new_font_by_name("Arial", 15.0)
                .build().unwrap(), false)
        };

        if let Ok(layout) = ctx
            .text()
            .new_text_layout(&font, &format!("Data: {} Field: {} Font: {} Found: {}", data, self.text, self.font_name, found) ,
                             200.0)
            .build() {

            let fill_color = Color::BLACK;
            if let Some(metric) = layout.line_metric(0) {
                ctx.draw_text(&layout, (0.0, metric.height), &fill_color);
             }
        }
    }
}

struct BindingScrollOffsets<T>{
    name: String,
    phantom_t: PhantomData<T>
}

impl<T> BindingScrollOffsets<T> {
    pub fn new(name: String) -> Self {
        BindingScrollOffsets {name, phantom_t: Default::default() }
    }
}


impl <T, W: Widget<T> > Binding<InnerState, Scroll<T, W>> for BindingScrollOffsets<T>{
    type Change = (); // No point copying the offsets in here, they are cheap to get off the scroll

    fn apply_data_to_controlled(&self, data: &InnerState, controlled: &mut Scroll<T, W>, ctx:  &mut UpdateCtx) {
       // log::info!("Applied data to controlled {} {:?}", self.name , data.offsets);
        controlled.scroll_to( &mut |command, target| {}, data.offsets, ctx.size());
        ctx.request_paint();
    }

    fn append_change_required(&self, controlled: &Scroll<T, W>, data: &InnerState, change: &mut Option<Self::Change>) {

        if !controlled.offset().same( &data.offsets ){
            //log::info!("Change needed from {} Contained:{:?}  Data:{:?}", self.name, controlled.offset(), data.offsets);
            *change = Some(())
        }
    }

    fn apply_change_to_data(&self, controlled: &Scroll<T, W>, data: &mut InnerState, change: Self::Change, ctx: &mut EventCtx) {
        log::info!("Applied change to data {}  {:?}", self.name , controlled.offset());
        data.offsets = controlled.offset().clone()
    }
}


fn build_widget() -> impl Widget<OuterState> {
    let row = Flex::row()
        .with_child( TextBox::new().lens(OuterState::name))
        .with_child( TextBox::new().lens(OuterState::job));

    let scope = Scope::new(InnerState::new,
                           InnerState::text,
                           build_inner_widget()).lens(OuterState::job);

    row.with_child(scope)
}

fn build_inner_widget() -> impl Widget<InnerState> {
    let mut row = Flex::row();

    let leader = Scroll::new(make_col(0)).lens(InnerState::text);
    let leader = BindingHost::new(leader, BindingScrollOffsets::new("leader".into()), );

    let follower = Scroll::new(make_col(1)).lens(InnerState::text);
    let follower = BindingHost::new(follower, BindingScrollOffsets::new("follower".into()));

    row.add_child(follower);

    row.add_child(leader);


    let lensed = LensedWidget::new("Arial".into(), "Stuff".into()).lens(InnerState::text);
    let binding =BindingHost::new(lensed,
                                  (LensBinding::new(InnerState::font, LensedWidget::font_name),
                                          DataToWidgetOnlyBinding( LensBinding::new(InnerState::text, LensedWidget::text ))));
    row.add_child( Flex::column().with_child(TextBox::new().lens(InnerState::font)).with_child(binding));


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
