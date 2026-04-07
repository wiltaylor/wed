use crate::layout::Pane;
use async_trait::async_trait;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Breakpoint {
    pub path: PathBuf,
    pub line: usize,
    pub enabled: bool,
}

#[derive(Default)]
pub struct DapBreakpointsPane {
    pub breakpoints: Vec<Breakpoint>,
    pub selected: usize,
}

impl DapBreakpointsPane {
    pub fn new() -> Self { Self::default() }
    pub fn set_breakpoints(&mut self, mut bps: Vec<Breakpoint>) {
        bps.sort_by(|a, b| a.path.cmp(&b.path).then(a.line.cmp(&b.line)));
        self.breakpoints = bps;
        self.selected = 0;
    }
}

#[async_trait]
impl Pane for DapBreakpointsPane {
    fn name(&self) -> &str { "dap_breakpoints" }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sorted_by_line() {
        let mut p = DapBreakpointsPane::new();
        p.set_breakpoints(vec![
            Breakpoint { path: PathBuf::from("a"), line: 5, enabled: true },
            Breakpoint { path: PathBuf::from("a"), line: 2, enabled: true },
            Breakpoint { path: PathBuf::from("a"), line: 9, enabled: false },
        ]);
        let lines: Vec<_> = p.breakpoints.iter().map(|b| b.line).collect();
        assert_eq!(lines, vec![2, 5, 9]);
    }
}
