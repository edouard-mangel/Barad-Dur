# Rewilding Software Engineering — Chapter Summaries

By Tudor Girba and Simon Wardley. Published on [Medium/feenk](https://medium.com/feenk).

---

## Chapter 1: Introduction

The book opens by framing software engineering as a discipline in continuous crisis — a crisis first named at the 1968 NATO Conference and still unresolved. The scale of the problem is concrete: a core banking system can contain tens of millions of lines of code (100× the Apollo moon landing codebase), yet we rely on the same approach of reading code manually to understand it.

The central argument is that the software industry behaves like the plastic industry: we keep building but can't recycle. "Legacy" is the only domain where the word has become a pejorative, yet it consumes the vast majority of engineering time and budget (the US federal government spends 78% of its IT budget just keeping existing systems running; average migration projects take 4 years and 74% fail to complete).

The proposed reframe: software engineering should be understood as a decision-making activity about systems too large to fully grasp. The book introduces **Moldable Development** as the systemic, humane solution — and the metaphor of "rewilding" as the guiding vision: restoring the ecosystem of tooling rather than forcing nature to conform to concrete channels.

---

## Chapter 2: How We Make Decisions

Developers spend more than half their time not writing code but reading artifacts (code, logs, traces) to figure out what to do next. Yet almost no one ever questions or optimizes this reading process itself.

The chapter uses **Wardley Maps** to visualize the decision-making chain: development experience → information synthesis → views of the system → conversation → exploration → assessment → decision. Each step is mapped against how "evolved" (commoditized vs. novel) current practices are.

The core diagnosis: today's tools are monolithic and generic. The same IDE used to investigate a hospital system is used for an online gambling platform — the equivalent of digging a mine shaft with kitchen appliances. The resulting "architecture diagrams" are essentially paintings: they document the author's perspective at a point in time, not the actual system. They go stale instantly in a continuous-deployment world.

The chapter introduces Moldable Development as the alternative: instead of manually synthesizing information with generic tools, you build small contextual tools tailored to the exact problem at hand. The authors argue this is not harder or more expensive than the manual alternative — it is fundamentally more efficient.

---

## Chapter 3: Questions and Answers

Building on Chapter 2, this chapter applies the **scientific method** to decision making: hypothesis → exploration → assessment → decision or refined hypothesis. The loop has two measurable components: **time to question (ttQ)** and **time to answer (ttA)**.

The striking observation: the industry carefully tracks DORA metrics (deployment frequency, change lead time, time to recover) but never measures ttQ or ttA. We optimize the action, never the decision process that produces the action.

The Moldable Development proposition: by investing in micro-tools specific to a problem, you dramatically reduce ttA. The analogy is automated testing — once done manually, now each test encodes contextual value that generic linters never could. The same logic applies to static analysis, observability (custom signals), algorithm visualizations, and architecture diagrams.

The key leverage: lowering ttA frees energy to ask more questions, dramatically increasing the chance of finding the *right* question. One or two manual investigations per week becomes hundreds. This demands two distinct skills: one for answering questions (technical/tool-building) and one for asking them (domain/business thinking).

---

## Chapter 4: Flexing Those Thinking Muscles

This chapter argues that **system explainability** — not code readability — is the true goal of software quality. Readability is a property of code alone; explainability depends on three levers simultaneously: the structure of the system, the tools used to extract information from it, and the skills of the person using those tools.

Two concrete case studies ground the argument: a multi-billion dollar company whose robotic wafer production system had become a black box (feenk made it understandable in one month where others had failed over years — by first spending three weeks building inspection tools); and a "stuck cursor" bug in Glamorous Toolkit that was open for six months, resolved only after building a specialized tool to inspect the Rust-level objects underneath the Pharo wrappers.

The chapter introduces the **"two wolves"** of the book's rewilding metaphor: (1) challenging the assumption that software engineering is about writing code rather than making decisions, and (2) challenging the assumption that we cannot optimize how we ask and answer questions. Both wolves serve the same goal: making systems explainable.

---

## Chapter 5: Different Folks for Different Strokes

This chapter bridges technical and business perspectives, explicitly addressing the non-technical stakeholder. It opens with an unusual apology to business leaders tempted to replace engineers with AI — arguing that the gap between the two roles can be closed, and both are necessary.

The chapter uses a rich example: a **restaurant management system** (Cozy Corner) that models communication flows between waiters, tables, rooms, kitchen and business as a domain-specific language. This demonstrates how a complex, highly variable business domain can be made describable — and manageable — through a language built to match the domain, rather than forcing the domain to conform to off-the-shelf software.

The deeper structural point draws on **Domain-Driven Design**: there are always two media in any engineering organization — where work is done (the screen) and where ideas are expressed (the whiteboard). Moldable Development closes the gap between these two, making the system itself the source of truth for discussions rather than separate, inevitably-drifting diagrams. Two roles emerge: the **facilitator** (technical person who builds the contextual tools) and the **stakeholder** (business person who uses those tools to ask questions and make decisions).

---

## Chapter 6: Myths We Tell Ourselves

The final available chapter systematically dismantles pervasive beliefs that prevent organizations from addressing the real challenges.

**Myth 1: Software engineering is about building functionality.** An experiment with seven teams all producing an identical calendar application shows that identical functionality can be implemented with radically different structures. When the environment changes (the chapter uses the provocative example of the French Revolutionary 10-day week, then the more practical example of splitting a monolith), the difficulty of adapting depends entirely on structure, not functionality. Legacy systems are simply structures stuck in the past. Solving legacy requires cheap refactoring, and cheap refactoring requires understanding the system — which requires tools.

**Myth 2: Refactoring is not a business problem.** Organizational knowledge has migrated from people's heads and process documents into the structure of software systems. This makes the inside of systems a direct business asset. A case study of a $10B+ retailer trying to modernize their customer database illustrates that accessing and transforming that encoded knowledge is precisely the kind of decision-making investment that determines whether a company can adapt — or gets stuck in legacy. Refactoring is not a technical indulgence; it is how organizations preserve and evolve the knowledge that runs them.
