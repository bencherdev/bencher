pub trait Mean {
    fn mean(array: Vec<Self>) -> Option<Self>
    where
        Self: std::iter::Sum + std::ops::Div<usize, Output = Self>,
    {
        if array.is_empty() {
            return None;
        }

        let length = array.len();
        let sum: Self = array.into_iter().sum();
        let mean = sum / length;

        Some(mean)
    }
}
