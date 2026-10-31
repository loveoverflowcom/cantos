// ADR 0054 D04/D05. Draft, finite sequential expectation, not a proof of RPC or SQL.
// One Work/Unit, two stable actors, one pending share, one group and one revocable grant.
module viewer_access {
 type Event = Start | Claim | Replay | Change | Assert
 type S = { published: bool, work: bool, unit: bool, active: bool, claimed: int,
   mailbox: int, actor: int, groupMember: bool, groupOwner: bool, revoked: bool, expired: bool, shown: bool }
 pure def visible(s:S): bool = s.published and (s.work and s.unit) and s.active and not(s.revoked) and not(s.expired)
   and (s.claimed == s.actor or (s.groupMember and s.groupOwner))
 pure def project(s:S): S = {...s,shown:visible(s)}
 pure val initial:S = {published:false,work:false,unit:false,active:true,claimed:0,mailbox:1,actor:1,
   groupMember:false,groupOwner:false,revoked:false,expired:false,shown:false}
 var state:S
 var before:S
 var last:Event
 action init = all {state'=initial,before'=initial,last'=Start}
 action change(s:S):bool = all {state'=project(s),before'=state,last'=Change}
 action claim = all {
   state'=project({...state,claimed:if (state.claimed == 0 and not(state.revoked) and not(state.expired)) state.mailbox else state.claimed}),
   before'=state,last'=Claim
 }
 action replay = all {state'=project(state),before'=state,last'=Replay}
 action publish = change({...state,published:true,work:true,unit:true})
 action revoke = change({...state,revoked:true})
 action reuseMailbox = change({...state,mailbox:2,actor:2})
 action expire = change({...state,expired:true})
 action disable = change({...state,active:false})
 action childOnly = change({...state,published:true,work:false,unit:true})
 action foreignGroup = change({...state,published:true,work:true,unit:true,groupMember:true,groupOwner:false})
 action ownGroup = change({...state,published:true,work:true,unit:true,groupMember:true,groupOwner:true})
 action step = any {claim,replay,publish,revoke,reuseMailbox,expire,disable,childOnly,foreignGroup,ownGroup}
 val workAndUnit = state.shown implies (state.published and state.work and state.unit and state.active)
 val bindingStable = (last == Claim and before.claimed != 0) implies state.claimed == before.claimed
 val revokedSticky = before.revoked implies (state.revoked and not(state.shown))
 val groupIsolation = (state.claimed == 0 and not(state.groupOwner)) implies not(state.shown)
 val admitted = state.shown and state.claimed == 1
 val reusedBound = last == Claim and state.mailbox == 2 and state.claimed == 1
 val replayAfterRevoke = last == Replay and before.revoked and state.revoked
 val foreignDenied = state.groupMember and not(state.groupOwner) and state.claimed == 0 and not(state.shown)
}
