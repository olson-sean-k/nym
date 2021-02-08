#[derive(Debug)]
pub struct Memoized<O, E, F>
where
    F: Fn() -> Result<O, E>,
{
    f: F,
    output: Option<O>,
}

impl<O, E, F> Memoized<O, E, F>
where
    F: Fn() -> Result<O, E>,
{
    pub fn get(&mut self) -> Result<&O, E> {
        if let Some(ref output) = self.output {
            Ok(output)
        }
        else {
            let output = (self.f)()?;
            self.output = Some(output);
            Ok(self.output.as_ref().unwrap())
        }
    }

    pub fn peek(&self) -> Option<&O> {
        self.output.as_ref()
    }

    pub fn take(self) -> Result<O, E> {
        let Memoized { f, output } = self;
        if let Some(output) = output {
            Ok(output)
        }
        else {
            f()
        }
    }

    pub fn drain(&mut self) -> Option<O> {
        self.output.take()
    }
}

impl<O, E, F> Clone for Memoized<O, E, F>
where
    O: Clone,
    F: Clone + Fn() -> Result<O, E>,
{
    fn clone(&self) -> Self {
        Memoized {
            f: self.f.clone(),
            output: self.output.clone(),
        }
    }
}

impl<O, E, F> From<F> for Memoized<O, E, F>
where
    F: Fn() -> Result<O, E>,
{
    fn from(f: F) -> Self {
        Memoized { f, output: None }
    }
}
