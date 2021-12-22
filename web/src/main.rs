use sycamore::prelude::*;

fn main() {
    sycamore::render(|| {
        let count = Signal::new(vec![1, 2]);

        view! {
            ul {
                Keyed(KeyedProps {
                    iterable: count.handle(),
                    template: |x| view! {
                        li { (x) }
                    },
                    key: |x| *x,
                })
            }
        }
    });
}
