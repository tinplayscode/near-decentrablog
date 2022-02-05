/*
 * Decentrablog
 *
 * Learn more about writing NEAR smart contracts with Rust:
 * https://github.com/near/near-sdk-rs
 *
 */

// To conserve gas, efficient serialization is achieved through Borsh (http://borsh.io/)
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::U64;
use near_sdk::{env, near_bindgen, setup_alloc, AccountId, Promise};
use near_sdk::collections::UnorderedMap;
use near_sdk::serde::{Serialize, Deserialize};

setup_alloc!();

// Structs in Rust are similar to other languages, and may include impl keyword as shown below
// Note: the names of the structs are not important when calling the smart contract, but the function names are
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Blog {
    owner: AccountId,
    user_posts: UnorderedMap<AccountId, Vec<U64>>,
    posts: UnorderedMap<U64, Post>,

    next_post_id: U64,
    total_posts: U64,
    next_comment_id: U64,
    total_comments: U64,
    next_donation_id: U64,
    total_donations: U64,
}

impl Default for Blog {
  fn default() -> Self {
    Self {
      owner: env::signer_account_id(),
      user_posts: UnorderedMap::new(b"user_posts".to_vec()),
      posts: UnorderedMap::new(b"posts".to_vec()),
      total_posts: U64::from(0),
      next_post_id: U64::from(0),
      total_comments: U64::from(0),
      next_comment_id: U64::from(0),
      total_donations: U64::from(0),
      next_donation_id: U64::from(0),
    }
  }
}

/// Implements both `serde` and `borsh` serialization.
/// `serde` is typically useful when returning a struct in JSON format for a frontend.
#[derive(Serialize, Deserialize, BorshDeserialize, BorshSerialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Post {
    pub post_id: U64,
    pub title: String,
    pub body: String,
    pub author: AccountId,
    pub created_at: u64,
    pub comments: Vec<Comment>,

    pub upvotes: Vec<AccountId>,
    pub downvotes: Vec<AccountId>,
    
    pub donation_logs: Vec<DonationLog>,
}

#[derive(Serialize, Deserialize, BorshDeserialize, BorshSerialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Comment {
    pub comment_id: U64,
    pub body: String,
    pub author: AccountId,
    pub created_at: u64,
}

#[derive(Serialize, Deserialize, BorshDeserialize, BorshSerialize)]
#[serde(crate = "near_sdk::serde")]
pub struct DonationLog {
    pub donation_id: U64,
    pub amount: u128,
    pub donor: AccountId,
    pub created_at: u64,
    pub message: String,
}

#[near_bindgen]
impl Blog {
    pub fn create_post(&mut self, title: String, body: String) {
        let post_id = U64::from(self.next_post_id.0);

        let post = Post {
            post_id,
            title,
            body,
            author: env::signer_account_id(),
            created_at: env::block_timestamp(),

            comments: vec![],
            upvotes: vec![],
            downvotes: vec![],
            donation_logs: vec![],
        };
        
        self.posts.insert(&post_id, &post);
        self.total_posts = U64::from(self.total_posts.0 + 1);
        self.next_post_id = U64::from(self.next_post_id.0 + 1);

        let title = post.title;

        // Use env::log to record logs permanently to the blockchain!
        env::log(format!("Post '{}' was created", title).as_bytes());
    }

    pub fn get_owner(&self) -> AccountId {
        self.owner.clone()
    }

    pub fn get_post(&self, post_id: U64) -> Post {
        self.posts.get(&post_id).unwrap()
    }

    pub fn get_posts(&self) -> Vec<Post> {
        let mut posts = Vec::new();
        for post_id in self.user_posts.get(&env::signer_account_id()).unwrap() {
            posts.push(self.posts.get(&post_id).unwrap());
        }
        posts
    }

    pub fn get_total_posts(&self) -> U64 {
        self.total_posts
    }

    pub fn delete_post(&mut self, post_id: U64) {
        assert_eq!(self.owner, env::signer_account_id(), "Only owner can delete posts");
        self.posts.remove(&post_id);
        self.total_posts = U64::from(self.total_posts.0 - 1);
    }

    pub fn comment(&mut self, post_id: U64, comment: String) {
        // Check if the post exists
        let post = self.posts.get(&post_id).unwrap();
        assert!(post.post_id == post_id, "Post does not exist");
        assert!(comment.len() >= 10, "Comment must be at least 10 characters long");

        let author = env::signer_account_id();
        let created_at = env::block_timestamp();

        let comment = Comment {
            comment_id: U64::from(self.next_comment_id.0),
            author,
            body: comment,
            created_at,
        };

        self.next_comment_id = U64::from(self.next_comment_id.0 + 1);
        self.total_comments = U64::from(self.total_comments.0 + 1);

        self.posts.get(&post_id).unwrap().comments.push(comment);
    }

    pub fn delete_comment(&mut self, post_id: U64, comment_id: U64) {
        // only owner can delete comments
        assert_eq!(self.owner, env::signer_account_id(), "Only owner can delete comments");

        // Check if the post exists
        let post = self.posts.get(&post_id).unwrap();
        assert!(post.post_id == post_id, "Post does not exist");
        let comment = post.comments.iter().find(|c| c.comment_id == comment_id);
        assert!(comment.is_some(), "Comment does not exist");
        
        self.posts.get(&post_id).unwrap().comments.remove(comment_id.0 as usize);
        self.total_comments = U64::from(self.total_comments.0 - 1);
    }

    #[payable]
    pub fn donate(&mut self, post_id: U64, amount: u128, message: String) {
        // Check if the post exists
        let post = self.posts.get(&post_id).unwrap();
        assert!(post.post_id == post_id, "Post does not exist");

        // check if the amount is valid
        assert!(amount >= 1, "Amount must be greater than 0");
        // enough balance
        assert!(env::account_balance() >= amount, "Not enough balance");


        // transfer NEAR to the post author
        let author = post.author;
        let amount = amount;
        
        Promise::new(author).transfer(amount).then(self.save_to_donation_log(post_id, amount, message));
    }

    fn save_to_donation_log(&mut self, post_id: U64, amount: u128, message: String) -> Promise {
        let donor = env::signer_account_id();
        let created_at = env::block_timestamp();

        let donation_log = DonationLog {
            donation_id: U64::from(self.next_comment_id.0),
            amount,
            donor,
            created_at,
            message,
        };

        self.next_comment_id = U64::from(self.next_comment_id.0 + 1);
        self.total_comments = U64::from(self.total_comments.0 + 1);

        self.posts.get(&post_id).unwrap().donation_logs.push(donation_log);

        let donor = env::signer_account_id();

        //Mark the promise as fulfilled by doing nothing
        Promise::new(donor)
    }

    pub fn get_next_post_id(&self) -> U64 {
        self.next_post_id
    }
}

/*
 * The rest of this file holds the inline tests for the code above
 * Learn more about Rust tests: https://doc.rust-lang.org/book/ch11-01-writing-tests.html
 *
 * To run from contract directory:
 * cargo test -- --nocapture
 *
 * From project root, to run in combination with frontend tests:
 * yarn test
 *
 */
#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::MockedBlockchain;
    use near_sdk::{testing_env, VMContext};

    // mock the context for testing, notice "signer_account_id" that was accessed above from env::
    fn get_context(input: Vec<u8>, is_view: bool) -> VMContext {
        VMContext {
            current_account_id: "alice_near".to_string(),
            signer_account_id: "npmrunstart_testnet".to_string(),
            signer_account_pk: vec![0, 1, 2],
            predecessor_account_id: "carol_near".to_string(),
            input,
            block_index: 0,
            block_timestamp: 0,
            account_balance: 0,
            account_locked_balance: 0,
            storage_usage: 0,
            attached_deposit: 0,
            prepaid_gas: 10u64.pow(18),
            random_seed: vec![0, 1, 2],
            is_view,
            output_data_receivers: vec![],
            epoch_height: 19,
        }
    }

    #[test]
        fn create_post() {
        let context = get_context(vec![], false);
        testing_env!(context);
        let mut contract = Blog::default();
        contract.create_post("This is the title".to_string(), "Lets go Brandon!".to_string());

        //log id
        env::log(format!("Debug here {}", contract.get_post(U64::from(0)).post_id.0).as_bytes());
        
        assert_eq!(
            "This is the title".to_string(),
            contract.get_post(U64::from(0)).title
        );
        assert_eq!(
            "Lets go Brandon!".to_string(),
            contract.get_post(U64::from(0)).body
        );
        assert_eq!(U64::from(1), contract.get_total_posts());
        assert_eq!(U64::from(0), contract.get_post(U64::from(0)).post_id);
    }

    #[test]
    fn delete_a_post_then_add_two_posts() {
        let context = get_context(vec![], false);
        testing_env!(context);
        let mut contract = Blog::default();
        contract.create_post("This is the title".to_string(), "Lets go Brandon!".to_string());
        contract.delete_post(U64::from(1));
        
        assert_eq!(U64::from(0), contract.get_total_posts());

        // add a post
        contract.create_post("This is the title".to_string(), "Lets go Brandon!".to_string());
        contract.create_post("This is the title".to_string(), "Lets go Brandon!".to_string());
        assert_eq!(U64::from(2), contract.get_total_posts());

        //next post id
        assert_eq!(U64::from(3), contract.get_next_post_id());
    }

    #[test]
    fn return_owner_account_id() {
        let context = get_context(vec![], false);
        testing_env!(context);
        let contract = Blog::default();
        assert_eq!(
            "npmrunstart_testnet".to_string(),
            contract.get_owner()
        );
    }
}
