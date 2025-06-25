use serde::{ Deserialize, Serialize };
use sui_types::base_types::ObjectID;

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct I32 {
  bits: u32,
}

impl I32 {
  pub fn value(&self) -> i32 {
    self.bits as i32
  }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct I128 {
  bits: u128,
}

impl I128 {
  pub fn value(&self) -> i128 {
    self.bits as i128
  }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OptionU64 {
  is_none: bool,
  v: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SkipList {
  id: ObjectID,
  head: Vec<OptionU64>,
  tail: OptionU64,
  level: u64,
  max_level: u64,
  list_p: u64,
  size: u64,
  random: Random,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Random {
  seed: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TypeName {
  name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SkipListNode<V> {
	/// The score of node.
	score: u64,
	/// The next node score of node's each level.
	nexts: Vec<OptionU64>,
	/// The prev node score of node.
	prev: OptionU64,
	/// The data being stored
	value: V,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DynamicField<Ty0, Ty1> {
	pub id: ObjectID,
	pub name: Ty0,
	pub value: Ty1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkedTable<K> {
  id: ObjectID,
  head: MoveOption<K>,
  tail: MoveOption<K>,
  size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkedTableNode<K, V> {
	prev: MoveOption<K>,
	next: MoveOption<K>,
	value: V
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveOption<Element> {
  vec: Vec<Element>,
}