---
name: "content-generation"
description: "Write presentation outlines, slide content and speaker notes using SCQA, pyramid and story structures. Use when drafting or enriching a PPT talk or organizing its logic."
description_zh: "使用 SCQA、金字塔与故事结构撰写 PPT 大纲、页面内容和演讲备注，适用于起草或充实演示文稿。"
version: "1.0.0"
source: agentz
---

# PPT Content Generation

Generate high-quality content, outlines and speaker scripts for presentations.

## When to Use

- The user needs a content outline for a presentation
- The user asks for a speech script or speaker notes
- Existing content needs to be expanded or enriched
- The user asks how to organize the logic of a deck
- The user needs cases, data or citations to support a point

## Core Capabilities

### 1. Content Structure Design

#### SCQA framework (Minto pyramid principle)

**S (Situation)**: describe the current background and establish common ground with the audience.
- Example: "As digital transformation accelerates..."

**C (Complication)**: point out the problem or challenge to draw attention.
- Example: "Yet 80% of enterprises hit a bottleneck during transformation..."

**Q (Question)**: state the core question to be solved and lead into the topic.
- Example: "How can we break through these barriers and transform successfully?"

**A (Answer)**: give the solution and the key message, then develop the argument and cases.
- Example: "Our three-step method helps companies..."

#### Three-act structure

**Act 1: Opening (10%)**
```
1. Hook: grab attention (story / data / question)
2. Background: set the scene
3. Preview: outline the main content
```

**Act 2: Body (70%)**
```
1. Key point 1
   - Argument 1
   - Argument 2
   - Supporting case

2. Key point 2
   - Argument 1
   - Argument 2
   - Supporting case

3. Key point 3
   - Argument 1
   - Argument 2
   - Supporting case
```

**Act 3: Closing (20%)**
```
1. Summary: recap the key points
2. Reinforce: restate the critical message
3. Action: make the next step explicit
```

### 2. Title Writing Techniques

#### Verb-driven titles
- "5 Ways to Boost Efficiency" rather than "Efficiency Methods"
- "How to Reduce Operating Costs" rather than "Operating Costs"
- "Key Strategies for Growth" rather than "Growth Strategy"

#### Numbered titles
- "3 Steps to Improve User Experience"
- "5 Trends Shaping the Future Market"
- "7 Principles for an Efficient Team"

#### Question titles
- "Why Do We Need to Change?"
- "How Do We Stand Out from the Competition?"
- "What Are the Key Success Factors?"

#### Contrast titles
- "Traditional Approach vs New Solution"
- "Current State vs Ideal State"
- "Before & After: The Power of Transformation"

### 3. Content Development Techniques

#### Pyramid principle
```
Key message (conclusion first)
├── Argument 1
│   ├── Supporting data
│   └── Case example
├── Argument 2
│   ├── Supporting data
│   └── Case example
└── Argument 3
    ├── Supporting data
    └── Case example
```

#### 5W2H analysis
- **What**: define the problem
- **Why**: analyze causes
- **Who**: stakeholders
- **When**: timeline
- **Where**: scope and setting
- **How**: the solution
- **How much**: cost and benefit

#### STAR method (storytelling)
- **Situation**: describe the context
- **Task**: the challenge
- **Action**: what was done
- **Result**: the outcome

### 4. Speaker Notes

Generate speaker notes for every slide.

#### Opening slide (slide 1)
```
Speaking time: 30 seconds
Emphasis: grab attention, build rapport
Tone: warm, confident
Tips:
- Open with a compelling story or number
- Make eye contact with the audience
- Briefly preview the talk
```

#### Content slides (middle)
```
Speaking time: 1-2 minutes per slide
Emphasis: key message, key data
Tone: professional, clear
Tips:
- State the conclusion first, then the arguments
- Use concrete cases to explain abstract ideas
- Ask questions at the right moments to keep engagement
- Repeat key data for emphasis
```

#### Closing slide (last)
```
Speaking time: 1 minute
Emphasis: recap of key points, call to action
Tone: firm, inspiring
Tips:
- Summarize 3-5 key points concisely
- State the next action clearly
- End with a strong closing line
- Leave time for Q&A
```

### 5. Content Templates by Deck Type

#### Product introduction
```markdown
1. Cover: product name, slogan
2. Problem: user pain points and needs
3. Solution: how the product solves the problem
4. Core features: 3-5 main features
5. Advantages: comparison with competitors
6. Use cases: typical scenarios
7. Customer stories: success stories
8. Technical architecture (for technical audiences)
9. Roadmap: future plans
10. Pricing: plans and prices
11. Q&A: frequently asked questions
12. Call to action: contact details, trial link
```

#### Project report
```markdown
1. Cover: project name, presenter
2. Agenda: overview of the report
3. Background: goals and importance
4. Progress: milestones and completion
5. Key results: quantified data
6. Challenges: problems and responses
7. Lessons learned: summary and reflection
8. Next steps: upcoming plans
9. Resource needs: people, budget
10. Risks: potential issues
11. Summary: recap of key points
12. Q&A: discussion and feedback
```

#### Training course
```markdown
1. Cover: course topic, instructor
2. Learning objectives: what learners will master
3. Course outline: preview of modules
4. Topic 1: concept + case
5. Hands-on demo: step-by-step
6. Exercise 1: reinforce topic 1
7. Topic 2: concept + case
8. Hands-on demo: step-by-step
9. Exercise 2: reinforce topic 2
10. Capstone case: combined application
11. Review: key takeaways
12. Further resources: extended learning
```

#### Business plan
```markdown
1. Cover: company name, vision
2. Problem and opportunity: market pain points
3. Solution: product / service
4. Market analysis: size and trends
5. Competitive landscape: competitor analysis
6. Business model: how the company makes money
7. Core team: founders and executives
8. Milestones: achievements so far
9. Growth strategy: market expansion plan
10. Financial projections: revenue and costs
11. Funding ask: amount and use of funds
12. Contact: investor relations
```

### 6. Content Enhancement Techniques

#### Let the data speak
```
Weak:   Our product is very popular
Strong: Our product gained 100,000 users in the past 6 months with an NPS of 85

Weak:   The market is growing fast
Strong: The market grows at a 35% CAGR and is expected to reach $50 billion by 2025

Weak:   We saved a lot of cost
Strong: Process optimization cut operating costs by 40%, saving $2 million a year
```

#### Persuade with cases
```
Template:
[Customer] faced [specific problem]. After adopting our [solution],
they achieved [specific result] within [time period], improving [metric] by [percentage/value].

Example:
A large retailer struggled with low inventory turnover. After deploying our smart
inventory system, turnover rose 50% in 3 months, stock-outs fell 30%, and annual
inventory cost dropped by $8 million.
```

#### Simplify complex ideas with analogies
```
Complex: Our system uses a distributed microservice architecture...
Analogy: Like an efficient courier network, where each station works independently yet in coordination

Complex: Machine learning trains models on large amounts of data...
Analogy: Like teaching a child to read: the more they see, the more they recognize

Complex: A blockchain is a decentralized distributed ledger...
Analogy: Like everyone in the class holding the same ledger, with every transaction recorded in sync
```

#### Move people with stories
```
Story structure:
1. Set the scene: introduce the protagonist and background
2. Challenge: describe the problem or dilemma
3. Turning point: introduce the solution
4. Overcoming difficulty: show the actions taken
5. Result and takeaway: explain the impact and meaning

Example:
"Two years ago, Manager Zhang's team spent 20 hours every week compiling sales
reports by hand, often working late into the night. They tried many tools, none
satisfactory. With our automated reporting system, one click now produces an
accurate analysis in 5 minutes. As Zhang put it: 'This tool gave us back the
meaning of our work, and more time to serve customers.'"
```

### 7. Content Optimization Checklist

#### Opening (slides 1-2)
- [ ] Compelling opening (story / data / question)
- [ ] Clear goal and value of the talk
- [ ] Preview of the structure
- [ ] Connection with the audience

#### Body
- [ ] Clear logic (summary-detail-summary structure)
- [ ] Clear viewpoints (conclusion first)
- [ ] Sufficient evidence (data + cases)
- [ ] Well layered (3-5 key points)
- [ ] Balanced detail (emphasis where it matters)

#### Every slide
- [ ] Short, strong title
- [ ] One-sentence main message
- [ ] A moderate number of points (3-5)
- [ ] Specific, quantified information (data / cases)
- [ ] Complete speaker notes (timing / points / tips)

#### Closing (last 2-3 slides)
- [ ] Summary of key points
- [ ] Reinforced key message
- [ ] Clear call to action
- [ ] Time left for Q&A
- [ ] Clear contact details

### 8. Common Content Problems and Fixes

#### Problem 1: Empty content
**Symptom**: lots of platitudes, little substance
**Fix**:
- Back every viewpoint with data or a case
- Replace abstract statements with concrete examples
- Offer actionable advice
- Quantify everything that can be quantified

#### Problem 2: Confused logic
**Symptom**: content jumps around and does not connect
**Fix**:
- Use SCQA or the three-act structure
- State the core message of each slide
- Add transition slides between sections
- Keep the overall narrative flowing

#### Problem 3: Information overload
**Symptom**: too much to say, the audience cannot remember it
**Fix**:
- Focus on 3-5 core points
- Cut secondary information
- One point per slide
- Apply the "Rule of Three"

#### Problem 4: No interaction
**Symptom**: one-way delivery, attention drops
**Fix**:
- Ask questions at key moments
- Invite the audience to share experience
- Add a quiz or poll
- Reserve discussion time

#### Problem 5: Weak ending
**Symptom**: rushed finish, no call to action
**Fix**:
- Summarize the key points clearly
- Give a clear next step
- End with a strong closing line
- Leave a lasting impression

## Content Generation Process

### Step 1: Requirements analysis
```
Ask the user:
1. What are the topic and goal of the deck?
2. Who is the audience? (background, role, needs)
3. What is the occasion and the length of the talk?
4. What core message should it convey?
5. Is there content that must be included?
```

### Step 2: Structure design
```
Design the structure from the requirements:
- Decide the slide count (from the talk length)
- Divide into sections (3-5 core topics)
- Allocate slides (2-5 per topic)
- Design transitions between sections
```

### Step 3: Content writing
```
Generate content slide by slide:
- Write the slide title (short and strong)
- Distill the core message (one sentence)
- Expand the supporting points (3-5)
- Add data and cases (specific and quantified)
- Write speaker notes (timing + points)
```

### Step 4: Optimize and iterate
```
Check and improve:
- Is the overall logic clear?
- Is the amount of information moderate?
- Are the key points prominent?
- Are the cases sufficient?
- Is the language concise?
```

## Quick Generation Tips

### Outline in 10 minutes
1. Fix the core message (30 seconds)
2. List 3-5 viewpoints (2 minutes)
3. Expand each viewpoint into 3 points (5 minutes)
4. Add the opening and closing (2 minutes)
5. Check the logical flow (30 seconds)

### Expansion patterns
- **What + Why + How**: what it is, why it matters, how to do it
- **Problem + Solution + Benefit**
- **Past + Present + Future**
- **Theory + Example + Practice**

### Emergency plan
When time is short:
1. Settle on 3 core viewpoints
2. Find one data point or case for each
3. Open with a question or a number
4. Close by summarizing the viewpoints and calling for action
5. Just make sure the logic is coherent

## Learning Resources

### Speaking references
- TED Talks (openings and storytelling)
- Apple keynotes (product presentation)
- Academic conference talks (professional expression)

### Writing
- *The Pyramid Principle* (logical thinking)
- *Made to Stick* (making ideas memorable)

Apply these techniques systematically to help users build clear, persuasive and engaging presentation content.
