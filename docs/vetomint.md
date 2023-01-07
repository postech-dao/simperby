# Vetomint

## Summary

Vetomint is a consensus algorithm
designed to meet the special needs of Simperby. As its name suggests, Vetomint
is a modification of Tendermint. It is designed to support
displacing a designated consensus leader for arbitrary reasons via voting.
Moreover, Vetomint is designed with the self-hosted nature of Simperby in mind,
thereby runs under long timeout intervals
without performance penalty,
so that validators 
can run their nodes only occasionally.
The modifications applied to Tendermint, and the resulting properties of
Vetomint are summarized below:

### Behaviors

1. Validators may cast nil prevotes before the timeout expiration to vote against
the round (either with an existing proposal or not).  
2. If a validator collects more than 5/6 of the
prevotes (either nil or non-nil ones), it immediately stops and casts a precommit
based on the votes it has collected so far.

### Properties

1. Assuming that all non-byzantine validators cast prevotes/precommits and the
messages are delivered before the timeout expiration, the system is never
blocked by timeout by possible disuptes.
2. The system is guaranteed to be safe if the byzantine voting power is less than $1/3$. i.e., The byzantine threshold for safety is $1/3$.
3. If the byzantine voting power is less than $1/6$ and all non-byzantine
validators cast non-nil prevote on a proposal, then the proposal will be
accepted.

## Motivation

The main goal of Vetomint is to give validators an option to displace the
designated consensus leader by a voting-like procedure.
Validators in Vetomint are allowed to vote on the displacement of the current
consensus leader, even before the block proposal.

The fundamental reason for this need is the adoption of the stable leader policy
of Simperby.  
That is, the leader is expected to remain the same (in contrast to the
round-robin based leader selection in typical PoS consensus protocols) unless it
is displaced explicitly.  
Since all validators of Simperby are designed to run their own nodes and
Simperby puts significant burden on proposers (see [protocol
overview](./protocol_overview.md)), the stable leader policy significantly
improves the usability.  
Unlike all other validators that turn on their node occassionally for voting,
the leader should stay its node turned on for on-demand block production and
managing the chatting service.  
Not all validators are willing to take this burden, so it is desirable to choose
a few volunteers and let them be the leaders, stably.

One major drawback of the stable proposer policy is the need for a procedure
explicitly changing the designated block proposer, in the case if it does
something wrong.
But do we really need voting to do this?  
The answer is yes, because the notion of being wrong is not always objective.  
For example, a proposer might censor specific agendas from being included in a
block, or refuse to propose a block even if there is an eligible agenda.  
These are inarguably malicious —or at least irresponsible— behaviors, but there
is no clear rule to determine them — if only few validators have recognized the
censored agendas, should we call it censorship?

It should be also noted that we cannot rely on timeout expiration for this
decision making.
This is because Simperby is designed to produce blocks on-demand and the timeout
interval is assumed to be long.  
Thus, the displacement vote should be performed at the stage of waiting for a
proposal.

## Modifications to Tendermint

To achieve the goal, we have applied several modifications to Tendermint, which
are summarized as follows:
(1) in Vetomint, validators can cast nil prevotes on syntactically valid blocks,
or even without the presence of a proposed block;
(2) if a validator receives more than 5/6 of the total prevotes, then it moves
to the next phase immediately without waiting for a timeout expiration.

### Nil Prevotes before Timeout Expiration

First, validators in Vetomint can cast nil prevotes before the timeout
expiration.
These nil prevotes are used to displace the current leader, or the block
proposer.
If a validator believes that the current leader is being irresponsible or maliciously
censoring agendas, then it may cast a nil prevote, no matter whether the leader
has proposed a block, or the proposed block is syntactically valid.
Accumulation of the nil prevotes will lead to the rejection of the proposal
(if exists), moving to the next round with a changed leader, which is not
different from Tendermint.

<!---
TODO: Elaborate on this.
The following text is fine for giving intuition, but contains subtle errors.
Intuitively speaking, abusing the nil prevote in the original Tendermint means
nothing more than having a wrong clock.
-->
The safety condition will be preserved by this modification. That is, no two
validators in the same height will decide on different block proposals.
Intuitively speaking, abusing the nil prevote in the original Tendermint means
nothing more than having a wrong clock. Under the asynchronous setting of
Tendermint, this does not affect the safety. We will discuss about the liveness
condition, that is, validators will eventually decide on a block proposal, later
in this text.

### Early Termination

The second modification to Tendermint that Vetomint includes is the ability for
*early termination* based on the total prevotes received.
In Vetomint, if a validator collects more than 5/6 of the prevotes (either nil
or non-nil ones), it immediately stops and casts a precommit based on the votes
it has collected so far.
In the following paragraphs, we explain the rationale behind this modification.
We will also illustrate that the choice of 5/6 threshold naturally implies the
byzantine threshold of 1/6.

Let's recall the relevant behavior of Tendermint to understand this decision.
In Tendermint, a validator in the prevote phase can move to the precommit phase,
skipping a timeout event, if it collects more than 2/3 of the non-nil prevotes in
the given round.
Assuming that all non-byzantine validators participate promptly without network
delay, this significantly optimizes the consensus.
However, Tendermint does not allow early termination based on the total number
of prevotes received, even if more than 2/3 of the prevotes (including nil
prevotes) have been collected.
This behavior is not problematic in the original Tendermint because the timeout
interval is typically short and votes from non-byzantine validators do not
split, as the acceptance of proposals are decided deterministically by a set of
predefined rules, not by validators' will.

Since we have assumed that validators in Vetomint may freely cast nil prevotes,
however, the lack of *early termination* becomes a severe issue.
For example, it might be the case where the half of validators have casted
non-nil prevotes and the other half have casted nil prevotes.
In such a case, if it were the original Tendermint, although all validators have
made decisions, validators can do nothihg but wait for the upcoming timeout
expiration, which can be intolerably long.

To prevent such issues, we add a new early termination rule to Tendermint.
If a validator collects more than 5/6 of the total prevotes, either nil or
non-nil ones, it makes progress to the next phase, depending on the prevotes
it has received so far.
To be specific, among the received prevotes, if non-nil prevotes are more than
2/3 of the total prevotes, the validator broadcasts a non-nil precommit, and
broadcasts a nill precommit otherwise.

The threshold of 5/6 is chosen carefully.
To demonstrate this, consider the following extreme case where the threshold is
2/3 (thresholds less than 2/3 or greater than 1 are meaningless).  
Suppose that an inarguably acceptable block is proposed, and all but one
malicious validator, who is trying to delay the consensus, have casted non-nil
prevotes.
In the desirable scenario, the malicious action should be effectively ignored.
However, we can easily see that the malicious validator has a high probability
of hindering the consensus simply by casting a nil prevote.
All validators that receive the nil prevote before the early termination will
(conservatively) assume that a quorum has been failed to met: each of them has
received slightly more than 2/3 of the total prevotes and one of the prevotes is
nil, which implies that the received non-nil prevotes is less than 2/3.
In short, 2/3 threshold makes the system too fragile to external attacks.

To further explain the importance of the choice of 5/6, we will present an
(informal) argument to show that it is the optimal threshold.
Let $x$ be the early termination threshold.
We will demonstrate that $x = 5/6$ is optimal.

First, the early termination rule is intended to prevent the consensus from
getting stuck by timeout, as long as all non-byzantine validators particpate
promptly.
To achieve this, the byzantine threshold must be no greater than $1-x$.
If it were greater, byzatine validators could hinder the consensus by simply not
broadcasting prevotes.

Second, we want to ensure that, if more than 2/3 of the validators have casted
non-nil prevotes, the proposal will be accepted regardless of the actions of
malicious validators or the way messages are transferred.
(The 2/3 condition is necessary for the safety of the subsequent Tendermint
phases.)
Suppose that 2/3 of the validators have casted non-nil prevotes, and suppose
further that $1-x$ byzantine validators falsely broadcast nil prevotes.
If $$(1-x) + (2/3 - \epsilon) \ge x$$ for some small $\epsilon > 0$, then a
validator that receives $1-x$ nil prevotes from byzantine validators and
$2/3 - \epsilon$ non-nil prevotes will early terminate and incorrectly
conclude that the leader has been displaced.
This is undesirable, so we must have   $$(1-x) + (2/3 - \epsilon) < x$$, which
simplifies to $x \ge 5/6$.
As we want to minimize the byzantine threshold as much as possible, the optimal
choice is $x = 5/6$, and the byzantine threshold would be $1-x = 1/6$.

Liveness is not ensured in Vetomint. However, this is not a design mistake of
Vetomint, but rather an inherent issue with the threshold-based decision making:
of course, if validators repeatedly displace leaders, then a block will not be
created at all.
However, we can still ensure that malicious validators cannot hinder the
consensus if all non-byzantine validators decide not to cast nil-prevote.
In other words, the system always makes progress and produces a new block
assuming that all non-byzantine validators cast non-nil prevotes and the
messages are delivered.
