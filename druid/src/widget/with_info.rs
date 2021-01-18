use crate::widget::prelude::*;
use std::collections::HashMap;
use crate::command::SelectorSymbol;
use std::any::Any;
use crate::Selector;

pub struct WithInfo<W>{
    widget: W,
    info: HashMap<SelectorSymbol, Box<dyn Any>>
}

impl <W> WithInfo<W>{
    pub fn new(widget: W) -> Self {
        WithInfo { widget, info: Default::default() }
    }

    pub fn with_info<I: 'static>(mut self, selector: Selector<I>, payload: I)->Self{
        self.info.insert(selector.symbol(), Box::new(payload) );
        self
    }
}

impl <T, W: Widget<T>> Widget<T> for WithInfo<W>{
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {
        self.widget.event(ctx, event, data, env)
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &T, env: &Env) {
        self.widget.lifecycle(ctx, event, data, env)
    }

    fn update(&mut self, ctx: &mut UpdateCtx, old_data: &T, data: &T, env: &Env) {
        self.widget.update(ctx, old_data, data, env)
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &T, env: &Env) -> Size {
        self.widget.layout(ctx, bc, data, env)
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &T, env: &Env) {
        self.widget.paint(ctx, data, env)
    }

    fn info_raw(&self, symbol: SelectorSymbol) -> Option<&dyn Any> {
        self.info.get(symbol).map(|x|&**x).or_else(||self.widget.info_raw(symbol))
    }
}