#[cfg(test)]
mod tests {
  use persist_memory::*;

  #[derive(Persist, Default)]
  struct Test1 {
    #[persist(0)] a: u32,
    b: u32,
    #[persist(1)] c: u32,
  }

  #[derive(Persist, Default)]
  struct Test2(#[persist(0)] u32, u32, #[persist(1)] u32);

  #[derive(Persist)]
  enum Test3 {
    VA,
    #[persist(0)] VB {
      #[persist(0)] a: u32,
      b: u32,
      #[persist(1)] c: u32,
    },
    #[persist(1)] VC(#[persist(0)] u32, u32, #[persist(1)] u32),
  }

  impl Default for Test3 {
    fn default() -> Self {
      Test3::VA
    }
  }
}
