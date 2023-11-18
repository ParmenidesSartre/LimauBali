***************************************************************
***        Engine Aggressiveness Statistics Tool V6.0       ***
***           (C) 2025, Stefan Pohl, www.sp-cc.de           ***
***************************************************************
***     This tool uses pgn-extract (C) David J. Barnes      ***
***        and some pgn-tools by (C) Norman Pollock         ***
***************************************************************

Engine Aggressiveness Statistics Tools (= EAS tools)

This batch-tools use the fantastic pgn-extract tool by David Barnes. It can be used on engine games 
as well as on human chessgames.
With these tools, the aggressiveness of engines (or human players) can be measured. 
Because a weaker player can be playing aggressive, too, the EAS-Score (= aggressivenes score, 
see explanation below) and all other statistics are build on percents from the won games of a 
engine/player, not on absolute numbers.

There are 2 tools: The EAS-tool and the Gauntlet EAS-tool: The EAS-tool evaluates all played games
in a source.pgn-file of all engines/players. The Gauntlet EAS-tool only evaluates the engine/player,
which played the most games in the source.pgn file. The Gauntlet EAS-tool is IMHO a good thing for
engine-developers, when they test their engine-dev-version vs. several opponents and are just
interested in the EAS-score of their own engine...

In both tools, you can set the "average length of all won games" parameter manually, instead of letting
the EAS-Tool calculating it (=hardcoding). This is useful, if (for example) the Gauntlet-EAS-Tool runs
on a database which leads to different value for this important parameter, compared to a full EAS-calculation
on a full gamebase, evaluating all played games.
Just open the Tool (.bat file) using any texteditor. In the beginning of the code, there are these 3 lines:
REM *** Change this number from 0 to set the average length of all wins to this hardcoded number
REM *** instead of letting the tool calculating it. This is useful for engine-developers...
set /A hard_moveaverage=0


EAS-Score is calculated with these rules:
1) Sacrifices: (percent*100) of the percent-values of the sacrifices (1-5+ pawnunits) calculated out 
of the won games by the engine, only. So, a weak engine (with a small number of won games) can get 
a high EAS-scoring, too, when the percent of sac-games in the won games is high (and the number of 
short wins). Higher pawnunits-sacs give bonus-points: 
1 pawnsac = 10x points  *** 2 pawnsac  = 50x points *** 3 pawnsac = 100x points 
4 pawnsac = 150x points *** 5+ pawnsac = 200x points *** 5+ Queensac = 250x points 

2) Very short won games (percent*100) of won games by the engine give these EAS-points:
60 moves= 18x points *** 55 moves= 27x points *** 50 moves= 42x points
45 moves= 68x points *** 40 moves= 100x points.  
Since V5.2, the move-limit is no longer fixed to 40-60 moves, but the average length of all won
games in the source.pgn is calculated. Reason is, that human games or adjucated engine games are 
much shorter than non-adjucated engine-games for example and the EAS-tools will now adjust the 
move-limits for short-win EAS-points to this "reality":

3) There is a new early sac bonus: If a sacrifice happens early in the game, the engine gets EAS-points:
(percent x 100 / 18)*(percent x 100 / 18). Early sac limit is: Average length of all wins in a database / 2.
percent means here: percents of early sacs in the pool of all found sacs of one engine. So, theoretically,
values of 0% or 100% can happen (but are very unlikely).
Additionally, if the average win game length of the engine is shorter than the average win game
length of all games in the source.pgn, the engine gets 5000 EAS-points for each move, their won 
games are shorter in average. These points are added to the early sac bonus, because I wanted to avoid
to make another category of points for this small bonus.

3) Bad draws: Bad draws are games, which were drawn before endgame (material check is done, the 
number of played moves does not matter) and draws after the engine had a material advantage of 
at least 1 pawn during a game, because the engine should win a game, if material was won. All
these bad draws are finally checked for a material disadvantage of at least 1 pawn: Because draws 
with material disadvantage prevented a possible loss and so, these games are no bad draws and are
not counted.
The formula for calculating the bad-draw EAS-points is a bit tricky:
a) The percent-value of all good draws (out of all draws, the engine played) by the engine 
is calculated (all draws - bad draws) and rounded: Example: Engine had 23.7% bad draws, then 
the value here is 76 (100% - 23.7% = 76.3% (good draws), then rounded).
b) This value is exp3. Means: 76*76*76 = 438976, then divided by 3000 = 146
c) This value is exp2. Means: 146*146 = 30625 
So, the engines gets 21316 EAS-points

Important: The EAS-tools need the bin-folder in the working-folder, because the bin-folder contains 
pgn-extract and some piece-pattern files, which are needed for the detection of sacrifices. 
Additionally, the tools store some temporary files in the bin-folder, while they are running. So do 
not change anything in the bin-folder and dont move it away and dont write-protect it!!!
And never start both tools or one tool more than once at the same time!!! If you want to do so, 
just copy the complete EAS-folder with the tools and the bin-folder in it and use both copies.
The EAS-tools run on one CPU-Thread, only, so, you can make several copies and then run several
instances of the tools simultaneously.

Of course, the EAS-Score is not a fixed value. It highly depends on the strength of players
and opponents. And in engine-tournaments it depends on thinking-time, opening-sets, PC-speed and
if the engine-games are adjusted by the GUI or played until mate.
But, IMO, the EAS-Score can be very helpful especially for engine-developers, which test their
engine-dev-versions (or new neural-nets) always in the same way and versus the same opponents.
In this case, the EAS-Score quickly shows progress or regress in aggressiveness...
And in a RoundRobin-tournament, the EAS-Score ratinglist (see the statistics_EAS_rating.txt file)
can be very interesting. Or on ratinglist-databases, a EAS-ratinglist shows, which engine play
aggressive or more solid (boring?)


When the tool is done, you will hear a short melody...

The EAS-tool writes a statistics_EAS_rating textfile, where all engines/players are ranked by their 
EAS-Score. 

Mention: The EAS-tools need at least 50 wins and 30 draws of one engine/player, otherwise a warning
is printed in the first ratinglist (behind the engine-name), that the EAS-score is not reliable...

The EAS-Tool writes 2 pgn-output databases: interesting_wins.pgn and errorgames.pgn

errorgames.pgn contains games with a Termination-Tag that suggests a non-regular game ending.
("anbandoned" for example). These games are sorted out before building the EAS-score, so these games
will not lead to distorted results.

interesting_wins.pgn contains: 
1) Queen Sacrifices, followed by
2) 5+ PawnUnit Sacrifices, followed by
3) 4 PawnUnit Sacrifices, followed by
4) 3 PawnUnit Sacrifices, followed by
5) 2 PawnUnit Sacrifices, followed by
6) 1 PawnUnit Sacrifices, followed by
7) Very short games, followed by
8) Games, ended before endgame (material) was reached, followed by
9) Games with material imbalance (Rook vs. Bishop and 2 pawns for example)

The games in the output-files are sorted in 2 ways:
First: The games are sorted by categories (category 1 is followed by category 2, 3, ... etc.).
Second: In each category, the games are sorted by length (0-19 moves, followed by 20-24 moves, followed by
35-29 moves... and so on, up to 120 moves and beyond). So, in each category, the shortest wins are at the
beginning and followed by the longer wins...

And, there are no double games in one output-file: If a game fits more than one category, it is stored
in the lowest category, all other apperances of this game in higher categories are deleted.
For example: A game contains a 3 PawnUnit-Sacrifice and is won before the endgame material is reached:
This game is stored in category 4 (= 3 PawnUnit Sacrifices) and not in category 8...

And each games gets a new Annotator-Tag, so it is clear, which category the game belongs to.
One of these 8 tags is added to each game-notation:

[Annotator "EAS-Tool: Queen Sacrifice found in this game"]
[Annotator "EAS-Tool: 5+ PawnUnits Sacrifice found in this game"]
[Annotator "EAS-Tool: 4 PawnUnits Sacrifice found in this game"]
[Annotator "EAS-Tool: 3 PawnUnits Sacrifice found in this game"]
[Annotator "EAS-Tool: 2 PawnUnits Sacrifice found in this game"]
[Annotator "EAS-Tool: 1 PawnUnit Sacrifice found in this game"]
[Annotator "EAS-Tool: Game was very short"]
[Annotator "EAS-Tool: Game ended before endgame (material)"]
[Annotator "EAS-Tool: Material imbalance found in this game"]


Mention, not to click with your mouse in the black window, where an EAS-tool is running. That freezes 
the processing. This is not a bug of the EAS-tool but a „feature“ of windows...

Enjoy!

Idea and all work done by Stefan Pohl (SPCC), pgn-extract binary compiled by Thomas Plaschke
www.sp-cc.de
(C) 2025, Stefan Pohl




