class PullBasedDistribution:
    "Constant Time Reward Distribution with Changing Stake Sizes and Required Gap"

    def __init__(self):
        # Per group
        self.total_stake = 0
        self.total_pending_stake = 0
        self.reward_per_token = 0
        self.reward_per_token_history = {}
        self.current_distribution_id = 0

        # Per account
        self.stake = {0x1: 0, 0x2: 0}
        self.reward_tally = {0x1: 0, 0x2: 0}
        self.pending_stake = {0x1: 0, 0x2: 0}
        self.distribution_id = {0x1: 0, 0x2: 0}

    def __update_state(self, address):
        "Ensure a valid state for the account before using it"
        if self.distribution_id[address] != self.current_distribution_id:
            self.stake[address] += self.pending_stake[address]
            self.reward_tally[address] += (self.pending_stake[address]
                                        * self.reward_per_token_history[self.distribution_id[address]])
            self.distribution_id[address] = self.current_distribution_id
            self.pending_stake[address] = 0

    def distribute(self, reward):
        "Distribute `reward` proportionally to active stakes"
        if self.total_stake > 0:
            self.reward_per_token += reward / self.total_stake

        prev_distribution_id = self.current_distribution_id
        self.reward_per_token_history[prev_distribution_id] = self.reward_per_token
        self.current_distribution_id += 1
        self.total_stake += self.total_pending_stake
        self.total_pending_stake = 0

    def deposit_stake(self, address, amount):
        "Increase the stake of `address` by `amount`"
        self.__update_state(address)

        self.pending_stake[address] += amount
        self.total_pending_stake += amount

    def withdraw_stake(self, address, amount):
        "Decrease the stake of `address` by `amount`"
        if amount > self.stake[address] + self.pending_stake[address]:
            raise Exception("Requested amount greater than staked amount")

        self.__update_state(address)

        pending_amount = min(amount, self.pending_stake[address])
        self.pending_stake[address] -= pending_amount
        self.total_pending_stake -= pending_amount

        computed_amount = amount - pending_amount
        self.stake[address] -= computed_amount
        self.reward_tally[address] -= self.reward_per_token * computed_amount
        self.total_stake -= computed_amount

    def compute_reward(self, address):
        "Compute reward of `address`. Inmutable"
        stake = self.stake[address]
        reward_tally = self.reward_tally[address]
        if self.distribution_id[address] != self.current_distribution_id:
            stake += self.pending_stake[address]
            reward_tally += self.pending_stake[address] * self.reward_per_token_history[self.distribution_id[address]]

        return stake * self.reward_per_token - reward_tally

    def withdraw_reward(self, address):
        "Withdraw rewards of `address`"
        self.__update_state(address)

        reward = self.compute_reward(address)

        self.reward_tally[address] = self.stake[address] * self.reward_per_token

        return reward

# Example
addr1 = 0x1
addr2 = 0x2

contract = PullBasedDistribution()

contract.deposit_stake(addr1, 100)
contract.deposit_stake(addr2, 50)

contract.distribute(0) # Still nothing to reward here

contract.withdraw_stake(addr1, 100)
contract.deposit_stake(addr1, 50)

contract.distribute(10)

# Expected to not be rewarded because the participant withdrawed stake before the second distribution (0)
print(contract.withdraw_reward(addr1))

# Expected to be rewarded with the entire first reward distribution (10)
print(contract.withdraw_reward(addr2))
