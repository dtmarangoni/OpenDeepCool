# UI/UX Feedback

This is the user feedback regarding the generated UI/UX.

## Refactors

### Removes

Parts and fields to be removed completely:
- Fans and Pump information: no fans or pump information should be displayed in UI. Neither it should be part of OpenDeepCool app. It shouldn't be measured in backend, it shouldn't have a service for it, and it shouldn't be part of plans, prd, prp, documents, etc. Remove everything related to fans and pump reading, measurements, displaying and mentioning everywhere in the project. ** EXCEPT FROM THE INFORMATION YOU GATHERED ON HOW TO MEASURE AND DISPLAY FANS AND PUMP INFO. KEEP IT IN SPEC FILES, SO IF WE NEED IT IN THE FUTURE YOU ALREADY HAVE THE INFORMATION.
- Remove the label bellow the big left square display "Shown on the cooler now".
- Remove the Display tab
- In configuration page / Startup / Start at boot - remove the second phrase "Each change asks for the administrator's password."

### Changes

Changes required:
- Make the window launch outside of tilling mode. And the window should be the minimum size to fit its content in width and height. So it will change depending of the monitor resolution. For small screens where the content would overflow the screen, add scrollbars.
- Move the app title to be centered in top bar.
- Change the configuration tab to a engine icon, and position it right sided. So the right side of top bar you will have the configuration engine icon + X icon for closing the app. In the top of configuration page, it should have a back icon, moving back to display page. If back should be left or right sided in configuration page, its up to you, follow UI/UX recommended guidelines. Don't put borders in back icon.
- Don't put borders in the X icon.
- In configuration page make the labels be close sided to its configuration and not filling the empty space. Right now the lab is on left and the configuration on the right, with a lot of empty space in between. Keep it left sided.
- Change the Refresh button to be just the refresh icon without borders. It should be inside the display page instead of top bar. It should be positioned in the top right side.
- Move the temperature selection C or F to be inside the configuration page under Display section. It should come after Rotation.
- the toggle selectors are too big, make them a little bit smaller.
- As the app already measures all sensors at once, the display page should how live graphs for each mini display squares. All should animate together.
- Center all displays modes in screen (center it horizontally and vertically) except for music mode. Right now some are centered and some ar right sided.
- For music keep the spectrum in the bottom and the modes as its right now. Only change the dancer. For the dancer we have agreed to center it horizontally and vertically, so it can show a good animation when hes squatting.
- If the center cant be achieved due to odd sided remaining columns, keep the last remaining to the left, so the figure will be a little bit right sided.

### Adds

- add a theme category inside the configuration. It should be the first in the list. It should contain:
- use system theme toggle. If its set, will disable the dark / light toggle, as it will follow the system theme. If active will use the current system theme, dark or light. If the system theme cannot be found, disable this put an warning info.
- dark / light theme toggle. Only enabled for use if the use system theme is not active. Will use OpenDeepCool default dark / light theme.

## Implementation steps

Create a single refactor tracking file for required changes. It should describe each change as a refactor task and should include the status if its completed or not.
1 - Start with removing tasks (**UI/UX agent** responsible)
2 - Then go to changing tasks (**UI/UX agent** responsible)
3 - After all tasks are finished, call UI/UX agent again to check if the app design as a whole match the best standards in terms of modern, minimal and clean UI. Use Anthropic Frontend Design as reference. (**UI/UX agent** responsible)
4 - If UI/Ux agents detect changes required due to bad design, let it apply. (**UI/UX agent** responsible)
4 - Review all UI app code:
- check if no leftovers remained after the removal and changes
- check if the UI App or project documentation should be updated.
- check if UI code comments should be updated, functions documentation, etc.
- check if the code is clean and well structured, following best practices in code modularity, reusability, maintainability and performance.
5 - Create a new build, run and ask the user to review.

### Each refactor task

For each refactor task to be done by **UI/UX agent**:
- update your context regarding the current UI app code and documents.
- update the general refactor tracking file with the task completed.

## Agents

- You should coordinate the tasks refactoring.
- A UI/UX agent should perform each task change and report to you when completed.

### UI/UX agent

- Should follow the best and modern UI/UX standards.
- Should follow Anthropic Frontend Design rules as reference.
