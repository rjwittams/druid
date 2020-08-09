use crate::{
    BoxConstraints, Data, Env, Event, EventCtx, LayoutCtx, Lens, LifeCycle, LifeCycleCtx, PaintCtx,
    Size, UpdateCtx, Widget,
};
use std::marker::PhantomData;

struct StateHolder<F: Fn(In) -> State, L: Lens<State, In>, In, State> {
    state: Option<State>,
    old_state: Option<State>,
    make_state: F,
    lens: L,
    phantom_in: PhantomData<In>,
}

impl<F: Fn(In) -> State, L: Lens<State, In>, In: Data, State: Data> StateHolder<F, L, In, State> {
    pub fn new(make_state: F, lens: L) -> Self {
        StateHolder {
            state: None,
            old_state: None,
            make_state,
            lens,
            phantom_in: Default::default(),
        }
    }

    fn ensure_state(&mut self, data: &In) {
        if self.state.is_some() {
            // This check is required as the borrow checker thinks the &mut borrows for the else branch too
            if let Some(state) = &mut self.state {
                self.lens.with_mut(state, |inner| {
                    if !inner.same(&data) {
                        *inner = data.clone()
                    }
                });
            } else {
                panic!("Unreachable: satisfy borrow checker");
            }
        } else {
            self.state = Some((self.make_state)(data.clone()));
        }
    }

    fn with_state<V>(&mut self, data: &In, mut f: impl FnMut(&State) -> V) -> V {
        self.ensure_state(data);
        f(self.state.as_ref().unwrap())
    }

    fn with_old_state_and_state<V>(
        &mut self,
        data: &In,
        mut f: impl FnMut(&State, &State) -> V,
    ) -> V {
        self.ensure_state(data);
        let state = self.state.as_ref().unwrap();
        let (os, ret) = match &mut self.old_state {
            Some(os) => (state.clone(), f(os, state)),
            None => {
                let temp_old_state = state.clone();
                let ret = f(&temp_old_state, state);
                (temp_old_state, ret)
            }
        };
        self.old_state = Some(os);
        ret
    }

    fn with_state_mut<V>(&mut self, data: &In, mut f: impl FnMut(&mut State) -> V) -> V {
        self.ensure_state(data);
        f(self.state.as_mut().unwrap())
    }

    fn write_back_input(&mut self, data: &mut In) {
        //log::info!("Writing back state value {:?} to Input {:?}" , self.state, data);
        if let Some(state) = &self.state {
            self.lens.with(state, |inner| {
                if !inner.same(&data) {
                    *data = inner.clone()
                }
            })
        }
    }
}

pub struct Scope<F: Fn(In) -> State, L: Lens<State, In>, In, State, W: Widget<State>> {
    sh: StateHolder<F, L, In, State>, // These parts are bundled away from inner in order to help the borrow checker.
    inner: W,
}

impl<F: Fn(In) -> State, L: Lens<State, In>, In: Data, State: Data, W: Widget<State>>
    Scope<F, L, In, State, W>
{
    pub fn new(make_state: F, lens: L, inner: W) -> Self {
        Scope {
            sh: StateHolder::new(make_state, lens),
            inner,
        }
    }
}

impl<F: Fn(In) -> State, L: Lens<State, In>, In: Data, State: Data, W: Widget<State>> Widget<In>
    for Scope<F, L, In, State, W>
{
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut In, env: &Env) {
        let holder = &mut self.sh;
        let inner = &mut self.inner;
        holder.with_state_mut(data, |state| inner.event(ctx, event, state, env));
        holder.write_back_input(data);

        // Because our input data may not have changed,
        // we have to call update - widget pod will not trigger it.
        // Effectively we are a contained app
        let mut update_ctx = UpdateCtx {
            state: ctx.state,
            widget_state: ctx.widget_state,
        };

        holder.with_old_state_and_state(data, |old_state, state| {
            inner.update(&mut update_ctx, &old_state, state, env)
        })
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &In, env: &Env) {
        let holder = &mut self.sh;
        let inner = &mut self.inner;
        holder.with_state(data, |state| inner.lifecycle(ctx, event, state, env))
    }

    fn update(&mut self, ctx: &mut UpdateCtx, _old_data: &In, data: &In, env: &Env) {
        let holder = &mut self.sh;
        let inner = &mut self.inner;

        holder.with_old_state_and_state(data, |old_state, state| {
            inner.update(ctx, &old_state, state, env)
        });
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &In, env: &Env) -> Size {
        let holder = &mut self.sh;
        let inner = &mut self.inner;
        holder.with_state(data, |state| inner.layout(ctx, bc, state, env))
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &In, env: &Env) {
        let holder = &mut self.sh;
        let inner = &mut self.inner;
        holder.with_state(data, |state| inner.paint(ctx, state, env));
    }
}
