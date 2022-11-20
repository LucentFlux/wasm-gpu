pub enum Recoverable<E1, S, E2> {
    SoftErr(E1, S),
    HardErr(E2),
}
