pub trait Taped<AgentType=usize> : Clone {
    type Operation;

    /// Synchronize your object against a tape
    fn replay(&mut self, tape: Vec<Self::Operation>);

    /// Generate and remove your current tape
    fn tape(&mut self) -> Vec<Self::Operation>;
}
