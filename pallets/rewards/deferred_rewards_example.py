class PullBasedDistribution:
    "Constant Time Deferred Reward Distribution with Changing Stake Sizes and Deferred Reward"

    def __init__(self):
        self.total_stake = 0
        self.reward_per_token = 0
        self.last_rate = 0
        self.lost_reward = 0
        self.current_distribution_id = 0
        self.stake = {0x1: 0, 0x2: 0}
        self.reward_tally = {0x1: 0, 0x2: 0}
        self.rewarded_stake = {0x1: 0, 0x2: 0}
        self.distribution_id = {0x1: 0, 0x2: 0}

    def __update_rewarded_stake(self, address):
        "Ensure the `rewarded_stake` contains the last rewarded stake"
        if self.distribution_id[address] != self.current_distribution_id:
            self.distribution_id[address] = self.current_distribution_id
            self.rewarded_stake[address] = self.stake[address]

    def distribute(self, reward):
        "Distribute `reward` proportionally to active stakes"
        if self.total_stake == 0:
            raise Exception("Cannot distribute to staking pool with 0 stake")

        self.last_rate = (reward + self.lost_reward) / self.total_stake
        self.reward_per_token += self.last_rate
        self.lost_reward = 0
        self.current_distribution_id += 1;

    def deposit_stake(self, address, amount):
        "Increase the stake of `address` by `amount`"
        self.__update_rewarded_stake(address)

        self.stake[address] += amount
        self.reward_tally[address] += self.reward_per_token * amount
        self.total_stake += amount

    def withdraw_stake(self, address, amount):
        "Decrease the stake of `address` by `amount`"
        if amount > self.stake[address]:
            raise Exception("Requested amount greater than staked amount")

        self.__update_rewarded_stake(address)

        unrewarded_stake = max(self.stake[address] - self.rewarded_stake[address], 0)
        unrewarded_amount = min(amount, unrewarded_stake)
        rewarded_amount = amount - unrewarded_amount
        lost_reward = rewarded_amount * self.last_rate

        self.stake[address] -= amount
        self.reward_tally[address] -= self.reward_per_token * amount - lost_reward
        self.total_stake -= amount

        self.rewarded_stake[address] -= rewarded_amount
        self.lost_reward += lost_reward

    def compute_reward(self, address):
        "Compute reward of `address`. Inmutable"
        previous_stake = self.rewarded_stake[address]
        if self.distribution_id[address] != self.current_distribution_id:
            previous_stake = self.stake[address]

        return (self.stake[address] * self.reward_per_token
                - self.reward_tally[address]
                - previous_stake * self.last_rate)

    def withdraw_reward(self, address):
        "Withdraw rewards of `address`"
        reward = self.compute_reward(address)
        self.reward_tally[address] += reward
        return reward

# Example
addr1 = 0x1
addr2 = 0x2

contract = PullBasedDistribution()

contract.deposit_stake(addr1, 100)
contract.deposit_stake(addr2, 50)

contract.distribute(10)

contract.withdraw_stake(addr1, 100)
contract.deposit_stake(addr1, 50)

contract.distribute(10)

# Expected to not be rewarded because the participant withdrawed stake before the second distribution (0)
print(contract.withdraw_reward(addr1))

# Expected to be rewarded with the third part of the reward (3.3)
print(contract.withdraw_reward(addr2))
