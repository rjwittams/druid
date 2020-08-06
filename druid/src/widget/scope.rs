use crate::{Lens, Widget, EventCtx, LifeCycle, PaintCtx, BoxConstraints, LifeCycleCtx, Size, LayoutCtx, Event, Env, UpdateCtx, Data};
use crate::widget::WidgetWrapper;
use std::marker::PhantomData;
use std::fmt::Debug;

struct StateHolder<F: Fn(In)->State, L:Lens<State, In>,  In, State>{
    state: Option<State>,
    old_state: Option<State>,
    make_state: F,
    lens: L,
    phantom_in: PhantomData<In>
}

impl<F: Fn(In) -> State, L: Lens<State, In>, In: Data + Debug, State: Data + Debug> StateHolder<F, L, In, State> {
    pub fn new(make_state: F, lens: L) -> Self {
        StateHolder { state: None, old_state: None, make_state, lens, phantom_in: Default::default() }
    }

    fn ensure_state(&mut self, data: &In) {
        if self.state.is_some(){ // This check is required as the borrow checker thinks the &mut borrows for the else branch too
            if let Some(state) = &mut self.state {
                self.lens.with_mut(state, |inner| {
                    if !inner.same(&data) {
                        *inner = data.clone()
                    }
                });
            }else{
                panic!("Unreachable: satisfy borrow checker");
            }
        }else{
            self.state = Some( (self.make_state)(data.clone()));
            // Safety - we just made sure its not none
        }
    }

    fn with_state<V>(&mut self, data: &In, mut f: impl FnMut(&State)->V )->V{
        self.ensure_state(data);
        f(self.state.as_ref().unwrap())
    }

    fn with_old_state_and_state<V>(&mut self, data: &In, mut f: impl FnMut(&State, &State)->V )->V{
        self.ensure_state(data);
        let mut cloned = false;
        if self.old_state.is_none(){
            self.old_state = Some(self.state.as_ref().unwrap().clone());
            cloned = true;
        }
        let ret = f(self.old_state.as_ref().unwrap(), self.state.as_ref().unwrap());
        if !cloned{
            self.old_state = Some(self.state.as_ref().unwrap().clone());
        }
        ret
    }

    fn with_state_mut<V>(&mut self, data: &In, mut f: impl FnMut(&mut State)->V )->V {
        self.ensure_state(data);
        let ret = f(self.state.as_mut().unwrap());
        ret
    }

    fn write_back(&mut self, data: &mut In) {
        //log::info!("Writing back state value {:?} to Input {:?}" , self.state, data);
        if let Some(state) = &self.state {
            self.lens.with(state, |inner|{
                if !inner.same(&data){
                    *data = inner.clone()
                }
            })
        }
    }
}


pub struct Scope<F: Fn(In)->State, L:Lens<State, In>,  In, State, W: Widget<State>>{
    sh : StateHolder<F, L, In, State>,
    inner: W
}

impl <F: Fn(In)->State, L:Lens<State, In>,  In, State, W: Widget<State>> WidgetWrapper<W, State> for Scope<F, L, In, State, W>{
    fn wrapped(&self) -> &W {
        &self.inner
    }

    fn wrapped_mut(&mut self) -> &mut W {
        &mut self.inner
    }
}

impl<F: Fn(In) -> State, L: Lens<State, In>, In: Data + Debug, State : Data + Debug, W: Widget<State>> Scope<F, L, In, State, W> {
    pub fn new(make_state: F, lens: L, inner: W) -> Self {
        Scope { sh: StateHolder::new(make_state, lens, ) , inner}
    }
}


impl<F: Fn(In) -> State, L: Lens<State, In>, In: Data + Debug, State: Data + Debug, W: Widget<State>>
    Widget<In> for Scope<F, L, In, State, W> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut In, env: &Env) {
        let holder = &mut self.sh;
        let inner = &mut self.inner;
        holder.with_state_mut(data, |state| inner.event(ctx, event, state, env));
        holder.write_back(data);

        // Because our input data never changed we have to call update...
        // Effectively we are a contained app
        let mut update_ctx = UpdateCtx{
            state: ctx.state,
            widget_state: ctx.widget_state
        };

        holder.with_old_state_and_state(data, |old_state, state| inner.update( &mut update_ctx, &old_state, state, env ) )
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &In, env: &Env) {
        let holder = &mut self.sh;
        let inner = &mut self.inner;
        holder.with_state(data, |state| inner.lifecycle(ctx, event, state, env))
    }

    fn update(&mut self, ctx: &mut UpdateCtx, old_data: &In, data: &In, env: &Env) {
        let holder = &mut self.sh;
        let inner = &mut self.inner;

        holder.with_old_state_and_state(data, |old_state, state| inner.update( ctx, &old_state, state, env ));
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &In, env: &Env) -> Size {
        let holder = &mut self.sh;
        let inner = &mut self.inner;
        holder.with_state(data, |state| inner.layout(ctx, bc, state, env))
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &In, env: &Env) {
        let holder = &mut self.sh;
        let inner = &mut self.inner;
        holder.with_state(data, |state|  inner.paint(ctx, state, env));
    }
}