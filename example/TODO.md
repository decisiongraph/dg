# Showing TODO items for users
User should have a command to show all incomplete tasks that they have.
First the ones with Due Date and then by the priority (assume that for example Requirements order defines their importance). This should be a separate `dg` subcommand.

# Format text bullet points with `dg fmt`
* They should start with capital letter.
* They should end with '.', '!' or '?'.
* They should not have extra whitespace between '*' and the first big letter

# Many relationships like `related` are vague
We should use something more direct like `causes` if the `ADR-001` is related to `INC-001` or `caused` if the `INC-001` is already in resolved.

Also the `ADR-001` is `related` to `OPP-001`. Is quite weird. Collaborate with me how we could make this relationships more clearer?

# Being able to render the docs into static sites
This would immensively help the current client projects to show the documentation in human readable way

# Build automatic 'tech roadmaps'
Make some sort of heuristics based on the proposed OPP, ADR, INC action items. For example ask LLM to estimate them. It should have multiple swimlanes because clients like swimlanes.

It should go both back and to the future (we can use git commits when estimating the past).

# The git commit hook from `dg` should force certain commit conventions
This would then make it easier to know if work is related to something.

Hopefully there would be a security policy too for example for automated version updates.

# Figure out how to link README.md into the rest of the docs
This should be onboarding document.
But there should also be team specific documents.

# Team specific documents
This should integrate directly with the CODEOWNERS file to show what parts of the system the team owns.

# Figure out how to link this project into privacy policy
All subprocessors should somehow be visible here

# Add the multiplayer mode to render the sites too
It would be neat if user has logged in that the documents could be viewed in the main platform.

# Research question: Can we somehow how doc-coverage as we have the automatic test-coverage?
It would help to notice areas which are not yet documented
