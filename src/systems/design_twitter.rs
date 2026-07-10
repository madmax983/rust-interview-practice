//! # Design Twitter
//!
//! **Problem Name:** Design Twitter (`LeetCode` 355)
//! **Difficulty:** Medium
//! **Link:** <https://leetcode.com/problems/design-twitter/>
//!
//! **Why this matters in Rust:**
//! This problem is a textbook example of "System Design in the Small". It forces you to model
//! complex relationships (following, posting) and aggregate data from multiple sources (news feed)
//! while respecting ownership rules. It demonstrates how `HashMap` and `BinaryHeap` can work together
//! to solve the "Merge K Sorted Lists" problem efficiently, and how to manage a global monotonic
//! clock for ordering events without data races (in a single-threaded context).
//!
//! ## Approach
//!
//! We need to support three main operations:
//! 1. `post_tweet`: O(1) - Store the tweet in a list associated with the user.
//! 2. `follow`/`unfollow`: O(1) - Update a set of followees for the user.
//! 3. `get_news_feed`: O(K log K) - Retrieve the 10 most recent tweets from the user and their followees,
//!    where K is the number of followees.
//!
//! **Data Structures:**
//! - `tweets`: `HashMap<UserId, Vec<(Timestamp, TweetId)>>`. We store tweets chronologically.
//!   Since we only ever append, the vectors remain sorted by timestamp (oldest to newest).
//! - `follows`: `HashMap<UserId, HashSet<UserId>>`. Allows O(1) check and modification of follow status.
//! - `time`: `u64`. A global monotonic counter to order tweets across different users.
//!
//! **Algorithm for News Feed:**
//! The core challenge is merging K sorted lists (one for the user + one for each followee) to find the top 10.
//! Since the lists are sorted by time, we use a **Max-Heap** (Priority Queue) approach:
//! 1. Identify all users to pull from (the user themselves + everyone they follow).
//! 2. Push the *most recent* tweet from each of these users into the heap.
//!    We need to store metadata in the heap: `(Timestamp, TweetId, UserId, IndexInVec)`.
//! 3. Pop the max (most recent) from the heap and add to our result list.
//! 4. From the user who owned that tweet, pick the *next most recent* tweet (decrement index) and push it to the heap.
//! 5. Repeat until we have 10 tweets or the heap is empty.
//!
//! This avoids sorting all tweets (which would be slow) and only touches the most recent ones.
//!
//! ## Alternative Approaches
//!
//! - **Pull vs Push Model**:
//!   - *Pull (Current)*: Compute feed on demand. Good for write-heavy systems (lots of tweets, fewer reads).
//!   - *Push (Fan-out)*: When a user tweets, push the ID to all followers' pre-computed feed lists.
//!     Good for read-heavy systems (like actual Twitter), but expensive for users with millions of followers (the "Justin Bieber problem").
//!
//! - **Database**:
//!   - In a real system, you'd use a DB. Here, we simulate it with in-memory `HashMaps`.

use std::collections::{BinaryHeap, HashMap, HashSet};

type UserId = i32;
type TweetId = i32;
type Timestamp = u64;

/// A simplified Twitter backend simulation.
pub struct Twitter {
    /// Maps User ID to a list of their tweets (Timestamp, `TweetId`).
    /// Vectors are strictly ordered by Timestamp (ascending).
    tweets: HashMap<UserId, Vec<(Timestamp, TweetId)>>,
    /// Maps User ID to a Set of User IDs they follow.
    follows: HashMap<UserId, HashSet<UserId>>,
    /// Global monotonic clock.
    time: Timestamp,
}

/// Helper struct for the Priority Queue.
/// We implement `Ord` based on `timestamp` to ensure the Max-Heap pops the most recent tweet.
#[derive(Debug, PartialEq, Eq)]
struct HeapItem {
    timestamp: Timestamp,
    tweet_id: TweetId,
    user_id: UserId,
    index: usize, // Index in the user's tweet vector
}

impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // We want Max-Heap based on timestamp (larger timestamp = newer = higher priority)
        self.timestamp.cmp(&other.timestamp)
    }
}

impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Twitter {
    /// Creates a new, empty Twitter instance.
    #[must_use] 
    pub fn new() -> Self {
        Self {
            tweets: HashMap::new(),
            follows: HashMap::new(),
            time: 0,
        }
    }

    /// Compose a new tweet.
    pub fn post_tweet(&mut self, user_id: UserId, tweet_id: TweetId) {
        self.time += 1;
        self.tweets
            .entry(user_id)
            .or_default()
            .push((self.time, tweet_id));
    }

    /// Retrieve the 10 most recent tweet IDs in the user's news feed.
    /// Each item must be posted by users who the user followed or by the user themselves.
    /// Tweets must be ordered from most recent to least recent.
    #[must_use] 
    pub fn get_news_feed(&self, user_id: UserId) -> Vec<TweetId> {
        // 1. Identify sources: The user themselves + their followees
        // RUST INSIGHT: We can use `std::iter::once` chained with the followees iterator
        // to treat them uniformly.
        let empty_set = HashSet::new();
        let followees = self.follows.get(&user_id).unwrap_or(&empty_set);

        // ⚡ BOLT OPTIMIZATION: Pre-allocate heap capacity to avoid reallocations.
        // We will insert at most 1 item per source (user + followees).
        let mut heap = BinaryHeap::with_capacity(followees.len() + 1);

        // We need to look at the user + followees
        let sources = std::iter::once(&user_id).chain(followees.iter());

        // 2. Initialize Heap
        for &source_id in sources {
            if let Some(user_tweets) = self.tweets.get(&source_id)
                && let Some(&(timestamp, tweet_id)) = user_tweets.last()
            {
                // Push the most recent tweet of this user
                heap.push(HeapItem {
                    timestamp,
                    tweet_id,
                    user_id: source_id,
                    index: user_tweets.len() - 1,
                });
            }
        }

        // 3. Extract top 10
        // ⚡ BOLT OPTIMIZATION: Pre-allocate feed vector capacity since we know the exact maximum size (10).
        let mut feed = Vec::with_capacity(10);
        while feed.len() < 10 {
            if let Some(item) = heap.pop() {
                feed.push(item.tweet_id);

                // 4. Push the next tweet from the same user (if any)
                if item.index > 0 {
                    let next_idx = item.index - 1;
                    if let Some(user_tweets) = self.tweets.get(&item.user_id) {
                        // GOTCHA: We must re-check existence, though strictly logic guarantees it exists.
                        // `get` returns Option, but we know user_id is valid.
                        // Using `unwrap` is safe here based on invariant, but let's be safe.
                        let (timestamp, tweet_id) = user_tweets[next_idx];
                        heap.push(HeapItem {
                            timestamp,
                            tweet_id,
                            user_id: item.user_id,
                            index: next_idx,
                        });
                    }
                }
            } else {
                // Heap empty
                break;
            }
        }

        feed
    }

    /// Follower follows a followee.
    /// If the operation is invalid, it should be a no-op.
    // `follower_id`/`followee_id` are the domain terms even though they look similar.
    #[allow(clippy::similar_names)]
    pub fn follow(&mut self, follower_id: UserId, followee_id: UserId) {
        // Prevent self-following in the explicit set (though logic usually handles it)
        // The problem description typically implies explicit follows.
        // But for `get_news_feed`, we always include `self`.
        // Storing self-follow is redundant but harmless.
        if follower_id == followee_id {
            return;
        }
        self.follows
            .entry(follower_id)
            .or_default()
            .insert(followee_id);
    }

    /// Follower unfollows a followee.
    /// If the operation is invalid, it should be a no-op.
    // `follower_id`/`followee_id` are the domain terms even though they look similar.
    #[allow(clippy::similar_names)]
    pub fn unfollow(&mut self, follower_id: UserId, followee_id: UserId) {
        if let Some(set) = self.follows.get_mut(&follower_id) {
            set.remove(&followee_id);
        }
    }
}

// RUST INSIGHT: Default implementation is useful for generic contexts.
impl Default for Twitter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_flow() {
        let mut twitter = Twitter::new();

        // User 1 posts a tweet (id 5)
        twitter.post_tweet(1, 5);

        // User 1's feed should contain 5
        let feed = twitter.get_news_feed(1);
        assert_eq!(feed, vec![5]);
    }

    #[test]
    fn test_follow_system() {
        let mut twitter = Twitter::new();

        // User 1 posts 5
        twitter.post_tweet(1, 5);

        // User 2 posts 6
        twitter.post_tweet(2, 6);

        // User 1 follows User 2
        twitter.follow(1, 2);

        // User 1 should see: [6, 5] (6 is newer)
        let feed = twitter.get_news_feed(1);
        assert_eq!(feed, vec![6, 5]);

        // User 2 should only see [6]
        let feed_2 = twitter.get_news_feed(2);
        assert_eq!(feed_2, vec![6]);
    }

    #[test]
    fn test_unfollow_system() {
        let mut twitter = Twitter::new();

        twitter.post_tweet(1, 5);
        twitter.post_tweet(2, 6);
        twitter.follow(1, 2);

        assert_eq!(twitter.get_news_feed(1), vec![6, 5]);

        twitter.unfollow(1, 2);

        // User 1 should strictly see their own tweets now
        assert_eq!(twitter.get_news_feed(1), vec![5]);
    }

    #[test]
    fn test_feed_ordering_complex() {
        let mut twitter = Twitter::new();

        // Interleaved posting
        twitter.post_tweet(1, 10); // t=1
        twitter.post_tweet(2, 20); // t=2
        twitter.post_tweet(1, 11); // t=3
        twitter.post_tweet(2, 21); // t=4

        twitter.follow(1, 2);

        // Should be: 21, 11, 20, 10
        let feed = twitter.get_news_feed(1);
        assert_eq!(feed, vec![21, 11, 20, 10]);
    }

    #[test]
    fn test_limit_10() {
        let mut twitter = Twitter::new();

        // Post 15 tweets
        for i in 1..=15 {
            twitter.post_tweet(1, i);
        }

        let feed = twitter.get_news_feed(1);
        assert_eq!(feed.len(), 10);
        // Should be 15 down to 6
        assert_eq!(feed, vec![15, 14, 13, 12, 11, 10, 9, 8, 7, 6]);
    }
}
