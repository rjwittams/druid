use crate::widget::prelude::*;
use std::any::{Any, TypeId};

pub struct Augmented<W, Aug> {
    widget: W,
    augment: Aug,
}

impl<W, Aug> Augmented<W, Aug> {
    pub fn new(widget: W, augment: Aug) -> Self {
        Augmented {
            widget,
            augment,
        }
    }
}

impl<T, W: Widget<T>, Aug: 'static> Widget<T> for Augmented<W, Aug> {
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

    fn augmentation_raw(&self, type_id: TypeId) -> Option<&dyn Any> {
        if TypeId::of::<Aug>() == type_id {
            Some(&self.augment)
        }else{
            self.widget.augmentation_raw(type_id)
        }
    }
}
