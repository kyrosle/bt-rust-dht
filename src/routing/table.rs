use std::{iter::Filter, net::SocketAddr, slice::Iter};

use crate::id::{NodeId, ID_LEN};

use super::{
  bucket::{self, Bucket},
  node::{Node, NodeHandle, NodeStatus},
};

pub const MAX_BUCKETS: usize = ID_LEN * 8;

/// Routing table containing a table of routing nodes as well
/// as the id of the local node participating in the dht.
pub struct RoutingTable {
  // The node_id will always fall within the range of
  // the last bucket in the buckets array.
  buckets: Vec<Bucket>,
  node_id: NodeId,
}

impl std::fmt::Debug for RoutingTable {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("RoutingTable")
      .field("buckets", &self.buckets)
      .finish()
  }
}

impl RoutingTable {
  /// Create a new RoutingTable with the given node id as our id.
  pub fn new(node_id: NodeId) -> Self {
    let buckets = vec![Bucket::new()];
    RoutingTable { buckets, node_id }
  }

  /// Return the node id of the RoutingTable.
  pub fn node_id(&self) -> NodeId {
    self.node_id
  }

  /// Iterator over the closest good nodes to the given node id.
  ///
  /// The closeness of nodes has a maximum granularity of a bucket.
  /// For most use cases this is fine since we will usually be performing
  /// lookups and aggregating a number of results equal to the size of a bucket.
  pub fn closest_nodes(&self, node_id: NodeId) -> ClosestNodes {
    ClosestNodes::new(&self.buckets, self.node_id, node_id)
  }

  pub fn get_nodes(&self) -> Vec<SocketAddr> {
    let mut nodes = Vec::with_capacity(self.buckets.len() * 8);
    for bucket in &self.buckets {
      for node in bucket.iter() {
        nodes.push(node);
      }
    }
    nodes.into_iter().map(|n| n.handle().addr).collect()
  }

  /// Number of good nodes in the RoutingTable.
  pub fn num_good_nodes(&self) -> usize {
    self
      .closest_nodes(self.node_id())
      .filter(|n| n.status() == NodeStatus::Good)
      .count()
  }

  /// Number of questionable nodes in the RoutingTable.
  pub fn num_questionable_node(&self) -> usize {
    self
      .closest_nodes(self.node_id())
      .filter(|n| n.status() == NodeStatus::Questionable)
      .count()
  }

  /// Iterator over all buckets in the routing table.
  pub fn buckets(&self) -> impl Iterator<Item = &Bucket> + ExactSizeIterator {
    self.buckets.iter()
  }

  /// Find an instance of the target node in the RoutingTable, if it exists.
  #[allow(unused)]
  pub fn find_node(&self, node: &NodeHandle) -> Option<&Node> {
    let bucket_index = self.bucket_index_for_node(node.id);
    let bucket = self.buckets.get(bucket_index)?;
    bucket.ping_able_nodes().find(|n| n.handle() == node)
  }

  /// Find a mutable reference to an instance of the target node in the RoutingTable, if it
  /// exists.
  pub fn find_node_mut<'a>(
    &'a mut self,
    node: &'_ NodeHandle,
  ) -> Option<&'a mut Node> {
    let bucket_index = self.bucket_index_for_node(node.id);
    let bucket = self.buckets.get_mut(bucket_index)?;
    bucket.ping_able_nodes_mut().find(|n| n.handle() == node)
  }

  fn bucket_index_for_node(&self, node_id: NodeId) -> usize {
    let bucket_index = leading_bit_count(self.node_id, node_id);

    // Check the sorted bucket
    if bucket_index < self.buckets.len() {
      // Got the sorted bucket
      bucket_index
    } else {
      // Grab the assorted bucket
      self
        .buckets
        .len()
        .checked_sub(1)
        .expect("no buckets present in RoutingTable - implementation error")
    }
  }

  /// Add the node to the RoutingTable if there is space for it.
  pub fn add_node(&mut self, node: Node) {
    // Doing some checks and calculations here, outside of the recursion.
    if node.status() == NodeStatus::Bad {
      return;
    }

    let num_same_bits = leading_bit_count(self.node_id, node.id());

    if num_same_bits != MAX_BUCKETS {
      self.bucket_node(node, num_same_bits);
    }
  }

  /// Recursively tries to place the node into some buckets.
  fn bucket_node(&mut self, node: Node, num_same_bits: usize) {
    let bucket_index = bucket_placement(num_same_bits, self.buckets.len());
    // Try to place in correct bucket and if the Bucket was full, try to split it.
    if !self.buckets[bucket_index].add_node(node.clone())
      && self.split_bucket(bucket_index)
    {
      // Bucket split successfully, try to add again.
      self.bucket_node(node, num_same_bits);
    }
  }

  /// Tries to split the bucket at the specified index.
  ///
  /// Returns false if the split cannot be performed.
  fn split_bucket(&mut self, bucket_index: usize) -> bool {
    if !can_split_bucket(self.buckets.len(), bucket_index) {
      return false;
    }

    // Implementation is easier if we just remove the whole bucket, pretty cheap to
    // copy and we can manipulate the new buckets while they are in the
    // RoutingTable already.
    let split_bucket = match self.buckets.pop() {
      Some(bucket) => bucket,
      None => {
        panic!("no buckets present in RoutingTable - implementation error")
      }
    };

    // Push two more buckets to distribute nodes between.
    self.buckets.push(Bucket::new());
    self.buckets.push(Bucket::new());

    // the situation in the last bucket:
    // - each leading bit may not same
    // - accepting the node which their ideal belonging index is overflow the total length
    for node in split_bucket.iter() {
      self.add_node(node.clone());
    }

    true
  }
}

/// Returns true if the bucket can be split.
fn can_split_bucket(num_buckets: usize, bucket_index: usize) -> bool {
  bucket_index == num_buckets - 1 && bucket_index != MAX_BUCKETS - 1
}

/// Number of leading bits that are identical between the local and the remote node ids.
///
/// Here we can also consider this as the closest distance between self node id and the remote node id.
pub fn leading_bit_count(local_node: NodeId, remote_node: NodeId) -> usize {
  (local_node ^ remote_node).leading_zeros() as usize
}

/// Take the number of leading bits that are the same between our node and the
/// remote node and calculate a bucket index for that node id.
fn bucket_placement(num_same_bits: usize, num_buckets: usize) -> usize {
  // The index that the node should be placed in `eventually`, meaning
  // when we create enough buckets for that bucket to appear.
  let ideal_index = num_same_bits;

  if ideal_index >= num_buckets {
    num_buckets - 1
  } else {
    ideal_index
  }
}

// -------------------------- //

// Iterator filter for only good nodes.
type GoodNodes<'a> = Filter<Iter<'a, Node>, fn(&&Node) -> bool>;

// So that we are going to do here is iterate over every bucket
// in a hypothetically filled routing the tale(buckets slice).
// If the bucket we are interested in has not been created
// yet (not in the slice), go through the last bucket (assorted nodes)
// and check if any nodes would have been placed in that bucket.
// If we find one, return it and mark it in our assorted nodes array.
pub struct ClosestNodes<'a> {
  /// Received the reference from the `RoutingTable.buckets`.
  buckets: &'a [Bucket],
  /// A Iterator that contains the reference of good nodes.
  current_iter: Option<GoodNodes<'a>>,
  /// A sign used in Iterator iterating.
  current_index: usize,
  /// The ideal bucket index according to the `leading_bit_count`.
  start_index: usize,
  /// Since we could have assorted nodes that are interleaved between
  /// our sorted nodes as far as closest nodes are concerned, we need
  /// some way to hand the assorted nodes out and keep track of which
  /// ones we have handed out.
  ///
  /// `(The ideal Bucket Index, Node Reference, Returned Before)`
  ///
  /// these nodes are from the last bucket.
  assorted_nodes: Option<[(usize, &'a Node, bool); bucket::MAX_BUCKET_SIZE]>,
}

impl std::fmt::Debug for ClosestNodes<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Closest Nodes")
      .field("buckets", &self.buckets)
      .finish()
  }
}

impl<'a> ClosestNodes<'a> {
  fn new(
    buckets: &'a [Bucket],
    self_node_id: NodeId,
    other_node_id: NodeId,
  ) -> ClosestNodes<'a> {
    // the ideal bucket index(distance).
    // distance = the common prefix count of (self_node_id ^ target_node_id)
    let start_index = leading_bit_count(self_node_id, other_node_id);

    // filter the good nodes in ideal bucket.
    let current_iter = bucket_iterator(buckets, start_index);

    // these nodes are have assorted ideal bucket index(maybe) in the last bucket.
    let assorted_nodes = precomputed_assorted_nodes(buckets, self_node_id);

    ClosestNodes {
      buckets,
      current_iter,
      current_index: start_index,
      start_index,
      assorted_nodes,
    }
  }
}

impl<'a> Iterator for ClosestNodes<'a> {
  type Item = &'a Node;

  fn next(&mut self) -> Option<Self::Item> {
    let current_index = self.current_index;

    // Check if we have any nodes left in the `current iterator`.
    if let Some(ref mut iter) = self.current_iter {
      if let Some(node) = iter.next() {
        return Some(node);
      }
    }

    // The current iterator reach the end.
    // We have visited all the good nodes in the ideal bucket.

    // Check if we have any nodes to give in the `assorted bucket`.
    if let Some(ref mut nodes) = self.assorted_nodes {
      // filter out the good nodes.
      let mut nodes_iter = nodes.iter_mut().filter(|tup| is_good_node(&tup.1));

      // Find the node whose ideal bucket index is equal to current_index and having been not visited.
      if let Some(node) =
        nodes_iter.find(|tup| tup.0 == current_index && !tup.2)
      {
        node.2 = true;
        return Some(node.1);
      }
    }

    // Here the current_iterator of the current index has reach to the end.
    // The assorted nodes (its ideal bucket index is equal to current_index and has not been visited) have been visited.

    // Now it is time to advance the `current_index`.
    match next_bucket_index(MAX_BUCKETS, self.start_index, self.current_index) {
      Some(new_index) => {
        self.current_index = new_index;
        self.current_iter = bucket_iterator(self.buckets, self.current_index);

        // Recurse back into this function to check the previous code paths again.
        self.next()
      }
      None => None,
    }
  }
}

/// Optionally returns the precomputed positions for all assorted nodes.
fn precomputed_assorted_nodes(
  buckets: &[Bucket],
  self_node_id: NodeId,
) -> Option<[(usize, &Node, bool); bucket::MAX_BUCKET_SIZE]> {
  if buckets.len() == MAX_BUCKETS {
    return None;
  }

  // the ideal bucket index in last bucket of the buckets may not same, here will do some sort operation.
  let assorted_bucket = &buckets[buckets.len() - 1];
  // iterate the nodes from last bucket.
  let mut assorted_iter = assorted_bucket.iter().peekable();

  // So the bucket is not empty
  // and now we have a reference to initialize our stack allocated array.
  if let Some(&init_reference) = assorted_iter.peek() {
    // Set all tuples to true in case our bucket is not full.
    let mut assorted_nodes =
      [(0, init_reference, true); bucket::MAX_BUCKET_SIZE];

    for (index, node) in assorted_iter.enumerate() {
      // the ideal bucket index
      let bucket_index = leading_bit_count(self_node_id, node.id());

      assorted_nodes[index] = (bucket_index, node, false);
    }

    Some(assorted_nodes)
  } else {
    None
  }
}

/// Optionally returns the filter iterator for the bucket a the specified index.
///
/// Filter the good nodes(not including the last bucket(unsorted bucket table) because
/// that may contain assorted nodes(different ideal bucket index)).
fn bucket_iterator(buckets: &[Bucket], index: usize) -> Option<GoodNodes> {
  if buckets.len() == MAX_BUCKETS {
    buckets
  } else {
    &buckets[..(buckets.len() - 1)]
  }
  .get(index)
  .map(|bucket| good_node_filter(bucket.iter()))
}

/// Converts the given iterator into a filter iterator to return only good nodes.
fn good_node_filter(iter: Iter<Node>) -> GoodNodes {
  iter.filter(is_good_node)
}

/// Shakes first aat iterator making me take a double reference
/// (could avoid it by mapping, but not well)
fn is_good_node(node: &&Node) -> bool {
  let status = node.status();

  status == NodeStatus::Good || status == NodeStatus::Questionable
}

/// Computes the next bucket index that should be visited given the number
/// of buckets, and the current index.
///
/// Prevent Overflow.
///
/// Returns None if all the buckets have been visited.
fn next_bucket_index(
  num_buckets: usize,
  start_index: usize,
  current_index: usize,
) -> Option<usize> {
  // Since we prefer going right first, that means we are on the
  // right side then we want to go to the same offset on the left,
  // however, if we are on the left we want to go 1 past the offset
  // to the right. All assuming we can actually do this without going
  // out of bounds.
  match current_index.cmp(&start_index) {
    std::cmp::Ordering::Less => {
      let offset = (start_index - current_index) + 1;

      let right_index = start_index.checked_add(offset);
      let left_index = current_index.checked_sub(1);

      if index_is_in_bounds(num_buckets, left_index) {
        Some(left_index.unwrap())
      } else if index_is_in_bounds(num_buckets, right_index) {
        Some(right_index.unwrap())
      } else {
        None
      }
    }
    std::cmp::Ordering::Equal => {
      let right_index = start_index.checked_add(1);
      let left_index = start_index.checked_sub(1);

      if index_is_in_bounds(num_buckets, right_index) {
        Some(right_index.unwrap())
      } else if index_is_in_bounds(num_buckets, left_index) {
        Some(left_index.unwrap())
      } else {
        None
      }
    }
    std::cmp::Ordering::Greater => {
      let offset = current_index - start_index;

      let left_index = start_index.checked_sub(offset);
      let right_index = current_index.checked_add(1);

      if index_is_in_bounds(num_buckets, left_index) {
        Some(left_index.unwrap())
      } else if index_is_in_bounds(num_buckets, right_index) {
        Some(right_index.unwrap())
      } else {
        None
      }
    }
  }
}

/// Returns true if the overflow checked is in bounds of the given length.
fn index_is_in_bounds(length: usize, checked_index: Option<usize>) -> bool {
  match checked_index {
    Some(index) => index < length,
    None => false,
  }
}

// -------------------------- //

#[cfg(test)]
mod tests {
  use crate::id::{NodeId, NODE_ID_LEN};
  use crate::routing::bucket;
  use crate::routing::node::Node;
  use crate::routing::table::{self, RoutingTable};
  use crate::test;
  use pretty_assertions::assert_eq;

  #[test]
  fn positive_add_node_max_recursion() {
    let table_id = [1u8; NODE_ID_LEN];
    let mut table = RoutingTable::new(table_id.into());

    let mut node_id = table_id;
    // Modify the id so it is placed in the last bucket
    node_id[NODE_ID_LEN - 1] = 0;

    // Trigger a bucket overflow and since the ids are placed in the last bucket, all of
    // the buckets will be recursively created and inserted into the list of all buckets.
    let block_address =
      test::dummy_block_socket_address((bucket::MAX_BUCKET_SIZE + 1) as u16);
    for block_addr in block_address {
      let node = Node::as_good(node_id.into(), block_addr);

      table.add_node(node);
    }
  }

  #[test]
  fn positive_initial_empty_buckets() {
    let table_id = [1u8; NODE_ID_LEN];
    let table = RoutingTable::new(table_id.into());

    assert_eq!(table.buckets().count(), 1);
    for bucket in table.buckets() {
      assert_eq!(bucket.ping_able_nodes().count(), 0)
    }
  }

  #[test]
  fn positive_first_bucket_sorted() {
    let table_id = [1u8; NODE_ID_LEN];
    let mut table = RoutingTable::new(table_id.into());

    let mut node_id = table_id;
    // Flip first bit so we are placed in the first bucket
    node_id[0] |= 128;

    let block_address =
      test::dummy_block_socket_address((bucket::MAX_BUCKET_SIZE + 1) as u16);
    for block_addr in block_address {
      let node = Node::as_good(node_id.into(), block_addr);

      table.add_node(node);
    }

    // First bucket should be sorted
    assert_eq!(table.buckets().take(1).count(), 1);
    for bucket in table.buckets().take(1) {
      assert_eq!(bucket.ping_able_nodes().count(), bucket::MAX_BUCKET_SIZE)
    }

    // Assorted bucket should show up
    assert_eq!(table.buckets().skip(1).count(), 1);
    for bucket in table.buckets().skip(1) {
      assert_eq!(bucket.ping_able_nodes().count(), 0)
    }

    // There should be only two buckets
    assert_eq!(table.buckets().skip(2).count(), 0);
  }

  #[test]
  fn positive_last_bucket_sorted() {
    let table_id = [1u8; NODE_ID_LEN];
    let mut table = RoutingTable::new(table_id.into());

    let mut node_id = table_id;
    // Flip last bit so we are placed in the last bucket
    node_id[NODE_ID_LEN - 1] = 0;

    let block_address =
      test::dummy_block_socket_address((bucket::MAX_BUCKET_SIZE + 1) as u16);
    for block_addr in block_address {
      let node = Node::as_good(node_id.into(), block_addr);

      table.add_node(node);
    }

    // First buckets should be sorted (although they are all empty)
    assert_eq!(
      table.buckets().take(table::MAX_BUCKETS - 1).count(),
      table::MAX_BUCKETS - 1
    );
    for bucket in table.buckets().take(table::MAX_BUCKETS - 1) {
      assert_eq!(bucket.ping_able_nodes().count(), 0)
    }

    // Last bucket should be sorted
    assert_eq!(
      table.buckets().skip(table::MAX_BUCKETS - 1).take(1).count(),
      1
    );
    for bucket in table.buckets().skip(table::MAX_BUCKETS - 1).take(1) {
      assert_eq!(bucket.ping_able_nodes().count(), bucket::MAX_BUCKET_SIZE)
    }
  }

  #[test]
  fn positive_all_sorted_buckets() {
    let table_id = NodeId::from([1u8; NODE_ID_LEN]);
    let mut table = RoutingTable::new(table_id);

    let block_address =
      test::dummy_block_socket_address(bucket::MAX_BUCKET_SIZE as u16);
    for bit_flip_index in 0..table::MAX_BUCKETS {
      for block_addr in &block_address {
        let bucket_node_id = table_id.flip_bit(bit_flip_index);

        table.add_node(Node::as_good(bucket_node_id, *block_addr));
      }
    }

    assert_eq!(table.buckets().count(), table::MAX_BUCKETS);
    for bucket in table.buckets() {
      assert_eq!(bucket.ping_able_nodes().count(), bucket::MAX_BUCKET_SIZE)
    }
  }

  #[test]
  fn negative_node_id_equal_table_id() {
    let table_id = [1u8; NODE_ID_LEN];
    let mut table = RoutingTable::new(table_id.into());

    assert_eq!(table.closest_nodes(table_id.into()).count(), 0);

    let node = Node::as_good(table_id.into(), test::dummy_socket_addr_v4());
    table.add_node(node);

    assert_eq!(table.closest_nodes(table_id.into()).count(), 0);
  }
}
