use super::*;

type Ident = String;

#[derive(Debug, Clone)]
pub struct Port {
    pub(crate) name: Ident,
    pub(crate) ty: Type,
}

impl Port {
    pub fn name(&self) -> &Ident {
        &self.name
    }
}

#[derive(Debug, Clone)]
pub struct Regfile {
    pub(crate) name: Ident,
    pub(crate) width: u32,
    pub(crate) depth: u32,
}

impl Port {
    pub fn new(name: Ident, ty: Type) -> Self {
        Self { name, ty }
    }
}

#[derive(Debug, Clone)]
pub struct Module {
    pub(crate) name: Ident,
    pub(crate) inputs: Vec<Port>,
    pub(crate) outputs: Vec<Port>,
    pub(crate) body: Option<Vec<Stmt>>,
}

#[derive(Debug, Clone)]
pub struct Flow {
    pub(crate) name: Ident,
    pub(crate) kind: FlowKind,
    pub(crate) inputs: Vec<Port>,
    pub(crate) outputs: Vec<Port>,
    pub(crate) attrs: FlowAttributes,
    pub(crate) body: Option<Vec<Stmt>>,
}

impl Flow {
    pub fn inputs(&self) -> &[Port] {
        &self.inputs
    }
    pub fn outputs(&self) -> &[Port] {
        &self.outputs
    }
    pub fn body(&self) -> Option<&[Stmt]> {
        self.body.as_deref()
    }
    pub fn opcode(&self) -> Option<u32> {
        match self.attrs.opcode.as_ref() {
            Some(expr) => if let Expr::Lit(lit) = &**expr {
                Some(lit.value().try_into().unwrap())
            } else {
                None
            },
            None => None
        }
    }
}

impl Display for Flow {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "flow<{}> {}:", self.kind, self.name)?;
        writeln!(f, "  inputs: {:?}", self.inputs)?;
        writeln!(f, "  outputs: {:?}", self.outputs)?;
        writeln!(f, "  attrs: {}", self.attrs)?;

        if let Some(body) = &self.body {
            writeln!(f, "  body:")?;
            for stmt in body {
                writeln!(f, "    {}", stmt)?;
            }
        }

        Ok(())
    }
}

impl Display for FlowAttributes {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "(")?;
        if let Some(activator) = &self.activator {
            write!(f, "activator: {}, ", activator)?;
        }
        if let Some(funct3) = &self.funct3 {
            write!(f, "funct3: {}, ", funct3)?;
        }
        if let Some(funct7) = &self.funct7 {
            write!(f, "funct7: {}, ", funct7)?;
        }
        if let Some(opcode) = &self.opcode {
            write!(f, "opcode: {}, ", opcode)?;
        }
        write!(f, ")")
    }
}
impl Display for FlowKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            FlowKind::Default => write!(f, "default"),
            FlowKind::RType => write!(f, "rtype"),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FlowAttributes {
    pub(crate) activator: Option<Box<Expr>>,
    pub(crate) funct3: Option<Box<Expr>>,
    pub(crate) funct7: Option<Box<Expr>>,
    pub(crate) opcode: Option<Box<Expr>>,
}

impl FlowAttributes {
    pub fn from_tuples(tuples: Vec<(Ident, Option<Box<Expr>>)>) -> Self {
        let mut attr = Self::default();
        for (name, value) in tuples {
            match name.as_str() {
                "activator" => attr.activator = value,
                "funct3" => attr.funct3 = value,
                "funct7" => attr.funct7 = value,
                "opcode" => attr.opcode = value,
                _ => {}
            }
        }
        attr
    }

    pub fn with_activator(mut self, activator: Box<Expr>) -> Self {
        self.activator = Some(activator);
        self
    }
}

#[derive(Debug, Clone)]
pub enum FlowKind {
    Default,
    RType,
}


#[derive(Debug, Clone)]
pub struct Proc {
    pub(crate) consts:   Map<Ident, Box<Expr>>,
    pub(crate) regfiles: Map<Ident, Regfile>,
    pub(crate) modules:  Map<Ident, Module>,
    pub(crate) flows:    Map<Ident, Flow>,
}


impl Proc {
    pub fn get_flows<'a>(&'a self) -> std::collections::btree_map::Iter<'a, Ident, Flow> {
        self.flows.iter()
    }
    pub fn flows<'a>(&'a self) -> &'a Map<Ident, Flow> {
        &self.flows
    }
}

#[derive(Debug, Clone)]
pub enum ProcPart {
    Const {
        name: Ident,
        value: Box<Expr>,
    },
    Regfile(Regfile),
    Module(Module),
    Flow(Flow),
}

impl Proc {
    pub fn new() -> Self {
        Self {
            consts: Map::new(),
            regfiles: Map::new(),
            modules: Map::new(),
            flows: Map::new(),
        }
    }

    pub fn add_part(&mut self, part: ProcPart) {
        match part {
            ProcPart::Const { name, value } => {
                self.consts.insert(name, value);
            }
            ProcPart::Regfile(regfile) => {
                self.regfiles.insert(regfile.name.clone(), regfile);
            }
            ProcPart::Module(module) => {
                self.modules.insert(module.name.clone(), module);
            }
            ProcPart::Flow(flow) => {
                self.flows.insert(flow.name.clone(), flow);
            }
        }
    }

    pub fn from_parts(parts: Vec<ProcPart>) -> Self {
        let mut proc = Self::new();
        for part in parts {
            proc.add_part(part);
        }
        proc
    }

}