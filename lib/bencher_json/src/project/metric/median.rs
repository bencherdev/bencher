pub trait Median {
    #[expect(clippy::indexing_slicing, clippy::integer_division)]
    fn median(mut array: Vec<Self>) -> Option<Self>
    where
        Self:
            Copy + Clone + Ord + std::ops::Add<Output = Self> + std::ops::Div<usize, Output = Self>,
    {
        if array.is_empty() {
            return None;
        }

        array.sort_unstable();

        let size = array.len();
        if size.is_multiple_of(2) {
            let left = size / 2 - 1;
            let right = size / 2;
            Some((array[left] + array[right]) / 2)
        } else {
            Some(array[size / 2])
        }
    }
}
