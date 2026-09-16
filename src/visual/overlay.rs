use anyhow::{Result, bail};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayOperation {
    Show,
    Hide,
    Toggle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayRequest {
    pub operation: OverlayOperation,
    pub name: Option<String>,
}

pub fn handle(request: &OverlayRequest) -> Result<()> {
    let operation = match request.operation {
        OverlayOperation::Show => "show",
        OverlayOperation::Hide => "hide",
        OverlayOperation::Toggle => "toggle",
    };
    let name = request.name.as_deref().unwrap_or("current");
    bail!("overlay backend is not implemented in v0.1 (requested: {operation} '{name}')")
}
