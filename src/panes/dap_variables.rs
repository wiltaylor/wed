use crate::layout::Pane;
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct DapVariable {
    pub name: String,
    pub value: String,
    pub ty: Option<String>,
}

#[derive(Default)]
pub struct DapVariablesPane {
    pub variables: Vec<DapVariable>,
    pub selected: usize,
}

impl DapVariablesPane {
    pub fn new() -> Self { Self::default() }
    pub fn set_variables(&mut self, variables: Vec<DapVariable>) {
        self.variables = variables;
        self.selected = 0;
    }
}

#[async_trait]
impl Pane for DapVariablesPane {
    fn name(&self) -> &str { "dap_variables" }
}
