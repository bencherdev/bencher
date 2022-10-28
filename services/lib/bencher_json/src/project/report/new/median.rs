pub trait Median {
    fn median(mut array: Vec<Option<Self>>) -> Option<Self>
    where
        Self:
            Copy + Clone + Ord + std::ops::Add<Output = Self> + std::ops::Div<usize, Output = Self>,
    {
        if array.is_empty() {
            return None;
        }

        array.sort_unstable();

        let size = array.len();
        if (size % 2) == 0 {
            let left = size / 2 - 1;
            let right = size / 2;
            Some((array[left]? + array[right]?) / 2)
        } else {
            array[(size / 2)]
        }
    }
}
