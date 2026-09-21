Task 1 in this project's tracker has a stale description. We found the cause:
the post-login redirect reads `next` from an unvalidated query parameter, and
an absent one resolves to the string "null". Replace the task's description
with that explanation. Do not change anything else about the task.
