# ADR 003: Explicit workflow state

Status: accepted

Related asynchronous UI state is represented by discriminated reducer states. In particular, a running job owns both its identifier and latest status; they cannot be stored independently.

Temporary presentation state may remain local when it cannot contradict workflow phase. Effects synchronize with external systems, not one React state variable with another.
