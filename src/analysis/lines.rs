/// Byte-based line index built once per file. Looking up diagnostic positions
/// is logarithmic in the number of lines, without rescanning the source prefix.
pub(super) struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    pub(super) fn new(source: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(
            source
                .bytes()
                .enumerate()
                .filter_map(|(i, b)| (b == b'\n').then_some(i + 1)),
        );

        Self { starts }
    }

    pub(super) fn location(&self, source: &str, offset: usize) -> (usize, usize) {
        let line = self.starts.partition_point(|start| *start <= offset) - 1;
        (
            line + 1,
            source[self.starts[line]..offset].chars().count() + 1,
        )
    }
}
