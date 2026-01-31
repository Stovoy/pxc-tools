#[derive(Clone, Debug)]
pub struct InputSpec {
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct OutputSpec {
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct NodeSpec {
    pub type_name: String,
    pub inputs: Vec<InputSpec>,
    pub outputs: Vec<OutputSpec>,
}
