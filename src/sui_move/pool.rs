use anyhow::{anyhow, Error};
use serde::{ Deserialize, Serialize };
use sui_types::{ balance::Balance, base_types::ObjectID, object::Object };

use crate::sui_move::sui::{LinkedTable, I128};

use super::sui::{ I32, SkipList, TypeName };

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pool {
  pub id: ObjectID,
  pub coin_a: Balance,
  pub coin_b: Balance,
  pub tick_spacing: u32,
  pub fee_rate: u64,
  pub liquidity: u128,
  pub current_sqrt_price: u128,
  pub current_tick_index: I32,
  pub fee_growth_global_a: u128,
  pub fee_growth_global_b: u128,
  pub fee_protocol_coin_a: u64,
  pub fee_protocol_coin_b: u64,
  pub tick_manager: TickManager,
  pub rewarder_manager: RewarderManager,
  pub position_manager: PositionManager,
  pub is_pause: bool,
  pub index: u64,
  pub url: String,
}

impl TryFrom<&Object> for Pool {
  type Error = Error;

  fn try_from(object: &Object) -> Result<Self, Self::Error> {
    let move_object_opt = object.data.try_as_move();
		let Some(move_object) = move_object_opt else {
			return Err(anyhow!("Object is not MoveObject"));
		};

		let pool: Self = bcs::from_bytes(move_object.contents())?;

		Ok(pool)
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickManager {
  tick_spacing: u32,
  ticks: SkipList,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewarderManager {
  rewarders: Vec<Rewarder>,
  points_released: u128,
  points_growth_global: u128,
  last_updated_time: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rewarder {
  reward_coin: TypeName,
  emissions_per_second: u128,
  growth_global: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionManager {
  tick_spacing: u32,
  position_index: u64,
  positions: LinkedTable<ObjectID>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionInfo {
	position_id: ObjectID,
	liquidity: u128,
	tick_lower_index: I32,
	tick_upper_index: I32,
	fee_growth_inside_a: u128,
	fee_growth_inside_b: u128,
	fee_owned_a: u64,
	fee_owned_b: u64,
	points_owned: u128,
	points_growth_inside: u128,
	rewards: Vec<PositionReward>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionReward {
  growth_inside: u128,
  amount_owned: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tick {
	index: I32,
	sqrt_price: u128,
	liquidity_net: I128,
	liquidity_gross: u128,
	fee_growth_outside_a: u128,
	fee_growth_outside_b: u128,
	points_growth_outside: u128,
	rewards_growth_outside: Vec<u128>,
}