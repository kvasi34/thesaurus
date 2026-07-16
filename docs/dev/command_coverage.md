# Redis Command Coverage

Implementation status of every Redis 8.8 command, grouped by data type.

| Symbol | Meaning |
|--------|---------|
| ✓ | Yes |
| | Explicitly out of scope |

---

## Strings

| Command | Description | Implemented | Issue | Won't implement |
|---------|-------------|:-----------:|:-----:|:---------------:|
| `APPEND` | Appends a string to the value of a key | | | |
| `DECR` | Decrements the integer value of a key by one | | ✓ | |
| `DECRBY` | Decrements a number from the integer value of a key | | ✓ | |
| `DELEX` | Conditionally removes a key based on value or digest comparison | | | |
| `DIGEST` | Returns the XXH3 hash digest of a string value | ✓ | | |
| `GET` | Returns the string value of a key | ✓ | | |
| `GETDEL` | Returns the string value of a key and deletes the key | ✓ | | |
| `GETEX` | Returns the string value of a key and optionally sets its expiration | | | |
| `GETRANGE` | Returns a substring of the string stored at a key | | | |
| `GETSET` | Returns the previous value after setting a new one | | | |
| `INCR` | Increments the integer value of a key by one | | ✓ | |
| `INCRBY` | Increments the integer value of a key by a number | | ✓ | |
| `INCRBYFLOAT` | Increments the floating point value of a key by a number | | | |
| `INCREX` | Increments a key's value and sets its expiration | | | |
| `LCS` | Finds the longest common substring | | | |
| `MGET` | Returns the string values of one or more keys | | ✓ | |
| `MSET` | Sets the string values of one or more keys | | ✓ | |
| `MSETEX` | Sets multiple string keys with a shared expiration | | | |
| `MSETNX` | Sets the string values of keys only when none exist | | | |
| `PSETEX` | Sets the string value and expiration in milliseconds of a key | | | |
| `SET` | Sets the string value of a key | ✓ | | |
| `SETEX` | Sets the string value and expiration in seconds of a key | | | |
| `SETNX` | Sets the string value of a key only when the key doesn't exist | | | |
| `SETRANGE` | Overwrites part of a string value at a given offset | | | |
| `STRLEN` | Returns the length of a string value | | ✓ | |
| `SUBSTR` | Returns a substring of a string value | | | |

## Hashes

| Command | Description | Implemented | Issue | Won't implement |
|---------|-------------|:-----------:|:-----:|:---------------:|
| `HDEL` | Deletes one or more fields from a hash | | | |
| `HEXISTS` | Determines whether a field exists in a hash | | | |
| `HEXPIRE` | Sets expiry on a hash field in seconds | | | |
| `HEXPIREAT` | Sets expiry on a hash field as a Unix timestamp (seconds) | | | |
| `HEXPIRETIME` | Returns the expiry of a hash field as a Unix timestamp (seconds) | | | |
| `HGET` | Returns the value of a field in a hash | | | |
| `HGETALL` | Returns all fields and values in a hash | | | |
| `HGETDEL` | Returns a field's value and deletes it from the hash | | | |
| `HGETEX` | Returns field values and optionally sets their expiration | | | |
| `HINCRBY` | Increments the integer value of a hash field | | | |
| `HINCRBYFLOAT` | Increments the floating point value of a hash field | | | |
| `HKEYS` | Returns all fields in a hash | | | |
| `HLEN` | Returns the number of fields in a hash | | | |
| `HMGET` | Returns the values of multiple hash fields | | | |
| `HMSET` | Sets the values of multiple hash fields | | | |
| `HPERSIST` | Removes the expiry from hash fields | | | |
| `HPEXPIRE` | Sets expiry on a hash field in milliseconds | | | |
| `HPEXPIREAT` | Sets expiry on a hash field as a Unix timestamp (milliseconds) | | | |
| `HPEXPIRETIME` | Returns the expiry of a hash field as a Unix timestamp (ms) | | | |
| `HPTTL` | Returns the TTL of a hash field in milliseconds | | | |
| `HRANDFIELD` | Returns one or more random fields from a hash | | | |
| `HSCAN` | Iterates over fields and values of a hash | | | |
| `HSET` | Creates or modifies the value of a field in a hash | | | |
| `HSETEX` | Sets field values and optionally sets their expiration | | | |
| `HSETNX` | Sets a hash field only when it doesn't exist | | | |
| `HSTRLEN` | Returns the length of the value of a hash field | | | |
| `HTTL` | Returns the TTL of a hash field in seconds | | | |
| `HVALS` | Returns all values in a hash | | | |

## Lists

| Command | Description | Implemented | Issue | Won't implement |
|---------|-------------|:-----------:|:-----:|:---------------:|
| `BLMOVE` | Pops from one list and pushes to another; blocks if empty | | | |
| `BLMPOP` | Pops from the first non-empty list in a set of keys; blocks if all empty | | | |
| `BLPOP` | Removes and returns the first element of a list; blocks if empty | | | |
| `BRPOP` | Removes and returns the last element of a list; blocks if empty | | | |
| `BRPOPLPUSH` | Pops from one list and pushes to another; blocks if empty | | | |
| `LINDEX` | Returns an element from a list by its index | ✓ | | |
| `LINSERT` | Inserts an element before or after a pivot value in a list | | | |
| `LLEN` | Returns the length of a list | ✓ | | |
| `LMOVE` | Pops an element from one list and pushes it to another atomically | | | |
| `LMPOP` | Returns and removes elements from one of multiple lists | | | |
| `LPOP` | Returns and removes the first element(s) of a list | ✓ | | |
| `LPOS` | Returns the index of matching elements in a list | | | |
| `LPUSH` | Prepends one or more elements to a list | ✓ | | |
| `LPUSHX` | Prepends elements to a list only when the list exists | ✓ | | |
| `LRANGE` | Returns a range of elements from a list | ✓ | | |
| `LREM` | Removes occurrences of an element from a list | | | |
| `LSET` | Sets the value of an element in a list by its index | ✓ | | |
| `LTRIM` | Trims a list to the specified range | | | |
| `RPOP` | Returns and removes the last element(s) of a list | ✓ | | |
| `RPOPLPUSH` | Pops from the tail of one list and pushes to the head of another | | | |
| `RPUSH` | Appends one or more elements to a list | ✓ | | |
| `RPUSHX` | Appends elements to a list only when the list exists | ✓ | | |

## Sets

| Command | Description | Implemented | Issue | Won't implement |
|---------|-------------|:-----------:|:-----:|:---------------:|
| `SADD` | Adds one or more members to a set | ✓ | | |
| `SCARD` | Returns the number of members in a set | ✓ | | |
| `SDIFF` | Returns the difference of multiple sets | | | |
| `SDIFFSTORE` | Stores the difference of multiple sets in a key | | | |
| `SINTER` | Returns the intersection of multiple sets | | | |
| `SINTERCARD` | Returns the count of the intersection of multiple sets | | | |
| `SINTERSTORE` | Stores the intersection of multiple sets in a key | | | |
| `SISMEMBER` | Determines whether a member belongs to a set | | ✓ | |
| `SMEMBERS` | Returns all members of a set | ✓ | | |
| `SMISMEMBER` | Determines whether multiple members belong to a set | | ✓ | |
| `SMOVE` | Moves a member from one set to another atomically | ✓ | | |
| `SPOP` | Removes and returns one or more random members from a set | ✓ | | |
| `SRANDMEMBER` | Returns one or more random members from a set without removing them | | ✓ | |
| `SREM` | Removes one or more members from a set | ✓ | | |
| `SSCAN` | Iterates over members of a set | | | |
| `SUNION` | Returns the union of multiple sets | | | |
| `SUNIONSTORE` | Stores the union of multiple sets in a key | | | |

## Sorted Sets

| Command | Description | Implemented | Issue | Won't implement |
|---------|-------------|:-----------:|:-----:|:---------------:|
| `BZMPOP` | Removes and returns a member by score from one or more sorted sets; blocks if all empty | | | |
| `BZPOPMAX` | Removes and returns the highest-scoring member; blocks if empty | | | |
| `BZPOPMIN` | Removes and returns the lowest-scoring member; blocks if empty | | | |
| `ZADD` | Adds one or more members to a sorted set, or updates their scores | | | |
| `ZCARD` | Returns the number of members in a sorted set | | | |
| `ZCOUNT` | Returns the count of members with scores within a range | | | |
| `ZDIFF` | Returns the difference between multiple sorted sets | | | |
| `ZDIFFSTORE` | Stores the difference of multiple sorted sets in a key | | | |
| `ZINCRBY` | Increments the score of a member in a sorted set | | | |
| `ZINTER` | Returns the intersection of multiple sorted sets | | | |
| `ZINTERCARD` | Returns the count of the intersection of multiple sorted sets | | | |
| `ZINTERSTORE` | Stores the intersection of multiple sorted sets in a key | | | |
| `ZLEXCOUNT` | Returns the count of members within a lexicographical range | | | |
| `ZMPOP` | Removes and returns the highest- or lowest-scoring members from one or more sorted sets | | | |
| `ZMSCORE` | Returns the score of one or more members in a sorted set | | | |
| `ZPOPMAX` | Removes and returns the highest-scoring members from a sorted set | | | |
| `ZPOPMIN` | Removes and returns the lowest-scoring members from a sorted set | | | |
| `ZRANDMEMBER` | Returns one or more random members from a sorted set | | | |
| `ZRANGE` | Returns members within a range of indexes | | | |
| `ZRANGEBYLEX` | Returns members within a lexicographical range | | | |
| `ZRANGEBYSCORE` | Returns members within a range of scores | | | |
| `ZRANGESTORE` | Stores a range of members from a sorted set in a key | | | |
| `ZRANK` | Returns the index of a member ordered by ascending score | | | |
| `ZREM` | Removes one or more members from a sorted set | | | |
| `ZREMRANGEBYLEX` | Removes members within a lexicographical range | | | |
| `ZREMRANGEBYRANK` | Removes members within a range of indexes | | | |
| `ZREMRANGEBYSCORE` | Removes members within a range of scores | | | |
| `ZREVRANGE` | Returns members within a range of indexes in reverse order | | | |
| `ZREVRANGEBYLEX` | Returns members within a lexicographical range in reverse order | | | |
| `ZREVRANGEBYSCORE` | Returns members within a range of scores in reverse order | | | |
| `ZREVRANK` | Returns the index of a member ordered by descending score | | | |
| `ZSCAN` | Iterates over members and scores of a sorted set | | | |
| `ZSCORE` | Returns the score of a member in a sorted set | | | |
| `ZUNION` | Returns the union of multiple sorted sets | | | |
| `ZUNIONSTORE` | Stores the union of multiple sorted sets in a key | | | |

## Streams

| Command | Description | Implemented | Issue | Won't implement |
|---------|-------------|:-----------:|:-----:|:---------------:|
| `XACK` | Acknowledges messages in a consumer group | | | |
| `XACKDEL` | Acknowledges and conditionally deletes stream entries | | | |
| `XADD` | Appends a new message to a stream | | | |
| `XAUTOCLAIM` | Transfers ownership of pending messages in a consumer group | | | |
| `XCFGSET` | Sets IDMP configuration parameters for a stream | | | |
| `XCLAIM` | Transfers ownership of a pending message in a consumer group | | | |
| `XDEL` | Removes messages from a stream | | | |
| `XDELEX` | Deletes one or more entries from a stream | | | |
| `XGROUP CREATE` | Creates a consumer group | | | |
| `XGROUP CREATECONSUMER` | Creates a consumer in a consumer group | | | |
| `XGROUP DELCONSUMER` | Deletes a consumer from a consumer group | | | |
| `XGROUP DESTROY` | Destroys a consumer group | | | |
| `XGROUP SETID` | Sets the last-delivered ID of a consumer group | | | |
| `XIDMPRECORD` | Sets IDMP metadata on an existing stream message | | | |
| `XINFO CONSUMERS` | Returns a list of consumers in a consumer group | | | |
| `XINFO GROUPS` | Returns a list of consumer groups of a stream | | | |
| `XINFO STREAM` | Returns information about a stream | | | |
| `XLEN` | Returns the number of messages in a stream | | | |
| `XNACK` | Releases pending messages back to the group without acknowledging them | | | |
| `XPENDING` | Returns pending entries from a consumer group | | | |
| `XRANGE` | Returns messages within a range of IDs | | | |
| `XREAD` | Returns messages from one or more streams with IDs greater than requested | | | |
| `XREADGROUP` | Returns messages for a consumer in a group | | | |
| `XREVRANGE` | Returns messages within a range of IDs in reverse order | | | |
| `XSETID` | Sets the last ID of a stream | | | |
| `XTRIM` | Removes messages from the beginning of a stream | | | |

## Bitmaps

| Command | Description | Implemented | Issue | Won't implement |
|---------|-------------|:-----------:|:-----:|:---------------:|
| `BITCOUNT` | Counts the number of set bits in a string | | | |
| `BITFIELD` | Performs arbitrary bitfield integer operations on strings | | | |
| `BITFIELD_RO` | Performs read-only bitfield integer operations on strings | | | |
| `BITOP` | Performs bitwise operations on multiple strings and stores the result | | | |
| `BITPOS` | Finds the first set or clear bit in a string | | | |
| `GETBIT` | Returns the bit value at an offset | | | |
| `SETBIT` | Sets or clears the bit at an offset | | | |

## Arrays

| Command | Description | Implemented | Issue | Won't implement |
|---------|-------------|:-----------:|:-----:|:---------------:|
| `ARCOUNT` | Returns the number of non-empty elements in an array | | ✓ | |
| `ARDEL` | Deletes elements at specified indices | | ✓ | |
| `ARDELRANGE` | Deletes elements within a range of indices | | ✓ | |
| `ARGET` | Returns the value at a single index | | ✓ | |
| `ARGETRANGE` | Returns values within a range of indices | | ✓ | |
| `ARGREP` | Searches elements using textual predicates (EXACT, MATCH, GLOB, RE) | | | |
| `ARINFO` | Returns metadata about an array | | ✓ | |
| `ARINSERT` | Inserts values at consecutive indices using an auto-advancing cursor | | | |
| `ARLASTITEMS` | Returns the most recently inserted elements | | | |
| `ARLEN` | Returns the logical length of an array (max index + 1) | | ✓ | |
| `ARMGET` | Returns values at multiple indices in one call | | ✓ | |
| `ARMSET` | Sets values at multiple arbitrary indices | | ✓ | |
| `ARNEXT` | Returns the next index ARINSERT would use | | | |
| `AROP` | Performs aggregate operations on elements in a range | | | |
| `ARRING` | Inserts values into a fixed-size ring buffer with wrapping | | | |
| `ARSCAN` | Iterates existing elements in a range as index-value pairs | | | |
| `ARSEEK` | Repositions the ARINSERT/ARRING cursor | | | |
| `ARSET` | Sets one or more contiguous values starting at an index | | ✓ | |

## HyperLogLog

| Command | Description | Implemented | Issue | Won't implement |
|---------|-------------|:-----------:|:-----:|:---------------:|
| `PFADD` | Adds elements to a HyperLogLog key | | | |
| `PFCOUNT` | Returns the estimated cardinality of a HyperLogLog | | | |
| `PFDEBUG` | Internal debugging command for HyperLogLog values | | | |
| `PFMERGE` | Merges one or more HyperLogLog values into a single key | | | |
