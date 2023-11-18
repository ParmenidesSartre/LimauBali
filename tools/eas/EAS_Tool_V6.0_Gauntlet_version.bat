@echo off

REM *** Change this number from 0 to set the average length of all wins to this hardcoded number
REM *** instead of letting the tool calculating it. This is useful for engine-developers...
set /A hard_moveaverage=0

title EAS Tool V6.0 by Stefan Pohl  www.sp-cc.de
@echo ***************************************************************
@echo ***        Engine Aggressiveness Statistics Tool V6.0       ***
@echo ***           (C) 2025, Stefan Pohl, www.sp-cc.de           ***
@echo ***               XXXXXXXXXXXXXXXXXXXXXXXX                  ***
@echo ***               XXX Gauntlet-version XXX                  ***
@echo ***************************************************************
@echo ***     This tool uses pgn-extract (C) David J. Barnes      ***
@echo ***        and some pgn-tools by (C) Norman Pollock         ***
@echo ***************************************************************
@echo ***************************************************************

@echo Enter the name of your games pgn-file (without .pgn ending!) The file must be in the working folder!
@echo off

set /p filename=
set gamebase=%filename%.pgn

CALL :StartTimer

del errorgames.pgn 2>nul
del interesting_wins.pgn 2>nul

cd ./bin
del whitewins_cutoff.pgn 2>nul
del blackwins_cutoff.pgn 2>nul
del earlysacs.pgn 2>nul
del errorgames_collect.pgn 2>nul

del statistics_EAS_rating_work.txt 2>nul
del statistics_EAS_rating_work2.txt 2>nul
del statistics_EAS_rating_work3.txt 2>nul
del newsource.pgn 2>nul
del newsource_onlywins.pgn 2>nul
del enginegames.pgn 2>nul
del collect_sacgames_1.pgn 2>nul
del collect_sacgames_2.pgn 2>nul
del collect_sacgames_3.pgn 2>nul
del collect_sacgames_4.pgn 2>nul
del collect_sacgames_5.pgn 2>nul
del collect_sacgames_9.pgn 2>nul
del collect_shorts.pgn 2>nul

REM *** EAS single stats ***
del eas_singlestat_A.txt 2>nul
del eas_singlestat_B.txt 2>nul
del eas_singlestat_C.txt 2>nul
del eas_singlestat_D.txt 2>nul
del eas_singlestat_E.txt 2>nul
del eas_singlestat_F.txt 2>nul
del eas_singlestat_A_sorted.txt 2>nul
del eas_singlestat_B_sorted.txt 2>nul
del eas_singlestat_C_sorted.txt 2>nul
del eas_singlestat_D_sorted.txt 2>nul
del eas_singlestat_E_sorted.txt 2>nul
del eas_singlestat_F_sorted.txt 2>nul
del eas_singlestat_out.txt 2>nul


@echo *********************************************************************************************
@echo *** Check games, fix result-tags and delete all comments for faster computing
pgn-extract --quiet --fixresulttags -C -N -V --plycount ../%gamebase% --output newsource.pgn > NUL

@echo *********************************************************************************************
@echo *** Filter won games out of the source.pgn-file for calculating average game length of wins
pgn-extract --quiet -Tr1-0 -Tr0-1 newsource.pgn --output newsource_onlywins.pgn > NUL

REM *** Calculate average length of all won games in the source.pgn file, later needed for setting
REM *** the intervals of the length of the short games
CALL :moveaverage newsource_onlywins.pgn
set /A avg_length_all_wins=%ma_moveaverage%

REM *** Hardcoding override for moveaverage
if %hard_moveaverage% GTR 0 set /A avg_length_all_wins=%hard_moveaverage%

REM *** Calculate movelimit for short wins, depending on the average won game length of
REM *** all games of the source.pgn file
REM set /A shortwin_movelimit=(%avg_length_all_wins%/5)*5
REM set /A shortwin_movelimit-=15
set /A shortwin_movelimit=%avg_length_all_wins%-15

if %shortwin_movelimit% LSS 30 set /A shortwin_movelimit=30
if %shortwin_movelimit% GTR 95 set /A shortwin_movelimit=95

set /A sh_level1=%shortwin_movelimit%
set /A sh_level2=%shortwin_movelimit%-5
set /A sh_level3=%shortwin_movelimit%-10
set /A sh_level4=%shortwin_movelimit%-15
set /A sh_level5=%shortwin_movelimit%-20

REM *** Set Early sac limit. Half of the average length of all wins
set /A earlysac_limit=(%avg_length_all_wins%/2)
if %earlysac_limit% LSS 10 set /A earlysac_limit=10

REM **********************************************
REM *** XXXXX Gauntlet special operations XXXXXX
del outname* 2>nul
nameList newsource.pgn >NUL
CALL :find_gauntlet
set /a numb_engines=1
goto gauntletjump01
REM **********************************************

@echo *********************************************************************************************
@echo *** Counting number of engines in the pgn-file
REM *** Use Norm Pollock tool nameList to find all engine-names in the pgn-file ***
del outname* 2>nul
nameList newsource.pgn >NUL

REM *** Count number of lines in outname-file (= number of different engines in the source.pgn) ***
set /A numb_engines=0 
for /f %%a in (outname) do set /a numb_engines+=1

REM *** Read the names of all engines and put them in a string-array ***
set /A counter=1
:loop01
set /p engines[%counter%]=<outname
set /A counter=%counter%+1
if %counter% GTR %numb_engines% goto exitloop01
more +1 outname > outnamex 
set /p engines[%counter%]=<outnamex
more +1 outnamex > outname 
set /A counter=%counter%+1
if %counter% GTR %numb_engines% goto exitloop01
goto loop01
:exitloop01
del outname*

@echo *********************************************************************************************
@echo *** Number of engines found in the pgn-file: %numb_engines%
REM *** Start second timer for engine-processing. Needed for estimating the time to finish...
:gauntletjump01
CALL :StartTimer2

REM *** Start of processing all engines, one after another, in a loop ***
set /A engine_counter=1
:loop02

REM **********************************************
REM *** XXXXX Gauntlet special operations XXXXXX
goto gauntletjump02
REM **********************************************

del enginefile 2>nul 
if defined engines[%engine_counter%]  (
call echo "%%engines[%engine_counter%]%%">enginefile
set /p engine=<enginefile
del enginefile 2>nul 
)

:gauntletjump02
@echo XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX
@echo *** Processing engine %engine_counter%/%numb_engines% (%engine%)

@echo *********************************************************************************************
@echo *** Copy all games played by engine in several working files...
pgn-extract --quiet -Tp%engine% newsource.pgn --output enginegames.pgn > NUL
pgn-extract --quiet -Tr1/2-1/2 enginegames.pgn --output enginedraws.pgn > NUL

pgn-extract --quiet -Tr1-0 -Tw%engine% enginegames.pgn --output whitewins_nc.pgn > NUL

REM *** Check games for non regular game endings
pgn-extract --quiet --tagsubstr -t termination_error whitewins_nc.pgn --output errorgames.pgn> NUL
pgn-extract --quiet -cerrorgames.pgn -D -owhitewins.pgn whitewins_nc.pgn > NUL
pgn-extract --quiet errorgames.pgn -aerrorgames_collect.pgn
del errorgames.pgn 2>nul
del whitewins_nc.pgn 2>nul

pgn-extract --quiet -Tr0-1 -Tb%engine% enginegames.pgn --output blackwins_nc.pgn > NUL

REM *** Check games for non regular game endings
pgn-extract --quiet --tagsubstr -t termination_error blackwins_nc.pgn --output errorgames.pgn> NUL
pgn-extract --quiet -cerrorgames.pgn -D -oblackwins.pgn blackwins_nc.pgn > NUL
pgn-extract --quiet errorgames.pgn -aerrorgames_collect.pgn
del errorgames.pgn 2>nul
del blackwins_nc.pgn 2>nul

del allwins.pgn 2>nul
copy whitewins.pgn+blackwins.pgn allwins.pgn > NUL

@echo *********************************************************************************************
@echo|set /p="*** Counting number of wins: "
REM *** Calculate average length of all wins of the engine
CALL :moveaverage allwins.pgn
set /A avg_length_eng_wins=%ma_moveaverage%

if %ma_moveaverage% LEQ 9 (
@echo 00%ma_moveaverage% %engine% >>eas_singlestat_E.txt
goto moveavg_format)
if %ma_moveaverage% LEQ 99 (
@echo 0%ma_moveaverage% %engine% >>eas_singlestat_E.txt
goto moveavg_format)
if %ma_moveaverage% LEQ 999 (
@echo %ma_moveaverage% %engine% >>eas_singlestat_E.txt
goto moveavg_format)
:moveavg_format

REM *** Count number of won games by the engine ***
CALL :countgames allwins.pgn
set /A numb_wins=%c_numb_pgn%
@echo|set /p=%numb_wins%

@echo|set /p="...draws: "

REM *** Count number of draw games by the engine ***
CALL :countgames enginedraws.pgn
set /A numb_draws=%c_numb_pgn%
@echo %numb_draws%

REM *** If overall number of wins/draws is too small, print a warning behind the engine name in the
REM *** first ratinglist...
set /A warning=0
if %numb_wins% LSS 50 set /A warning=1
if %numb_draws% LSS 30 set /A warning=1
if %warning% GTR 0 @echo XXXXX WARNING: Not enough games, EAS-score not reliable [50+ wins and 30+ draws needed] XXXXX

set /A engine_eas=0
set /A eas_bad_draws=0
set /A eas_short_wins=0
set /A eas_sacs=0
set /A eas_earlysacs=0

REM *** Searching for "bad draws": Draws, which ended before endgame
REM *** or draws, where engine had more material than the opponent (1 pawns or more) in the game
@echo *********************************************************************************************
@echo|set /p="*** Searching for bad draws"

REM *** Find draws, which ended before endgame was reached (=bad draw)
pgn-extract --quiet -zno_endgame_draws enginedraws.pgn --output results.pgn >NUL
pgn-extract --quiet -cresults.pgn -D -obad_draws2.pgn enginedraws.pgn > NUL

REM *** Find all draws, were engine had material advantage (=bad draw)
pgn-extract --quiet -Tw%engine% -y1_pawnsac_black enginedraws.pgn -abad_draws2.pgn >NUL
pgn-extract --quiet -Tb%engine% -y1_pawnsac_white enginedraws.pgn -abad_draws2.pgn >NUL

REM *** Find draws, which ended before endgame, but were engine had material disadvantage (=no bad draw, delete these games)
pgn-extract --quiet -Tw%engine% -y1_pawnsac_white bad_draws2.pgn --output savedgames.pgn >NUL
pgn-extract --quiet -Tb%engine% -y1_pawnsac_black bad_draws2.pgn -asavedgames.pgn >NUL
pgn-extract --quiet -csavedgames.pgn -D -obad_draws.pgn bad_draws2.pgn > NUL

REM *** Delete possible double games
pgn-extract --quiet -D bad_draws.pgn --output bad_draws_final.pgn > NUL

REM *** Count number of bad draw games by the engine ***
CALL :countgames bad_draws_final.pgn
set /A numb_bad_draws=%c_numb_pgn%
CALL :percent %numb_draws% %numb_bad_draws%
set percent_bad_draws=%percent%

@echo %percent% %engine% >>eas_singlestat_F.txt

REM *** EAS-score is calculated out of the good draws:
REM *** (Percent of good draws exp3)/2500 and
REM *** this result exp2...

set /A numb_good_draws=%numb_draws%-%numb_bad_draws%
CALL :percent %numb_draws% %numb_good_draws%
set /A gd=%percentx100%/100

REM *** /2500 Alte Version 5.8
set /A temp_bd=(%gd%*%gd%*%gd%)/3000
set /A temp_bd=%temp_bd%*%temp_bd%

set /A engine_eas+=%temp_bd%
set /A eas_bad_draws=%engine_eas%


@echo|set /p="...short wins"
pgn-extract --quiet -bu%sh_level1% allwins.pgn --output short_mvs_wins.pgn > NUL
pgn-extract --quiet -bu%sh_level4% short_mvs_wins.pgn -acollect_shorts.pgn > NUL

REM *** Old 40 move limit is now sh_level5, 45 is sh_level4, 50 is sh_level3, 55 is sh_level2
REM *** 60 is sh_level1, but there is no need for renaming

set /A eas_singlestat_C_temp=0
REM *** Count number of wins of the engine, not longer than 40 moves ***
pgn-extract --quiet -bu%sh_level5% short_mvs_wins.pgn --output results.pgn > NUL
CALL :countgames results.pgn
set /A numb_shortwins=%c_numb_pgn%
set /A won_40_mvs=%numb_shortwins%
CALL :percent %numb_wins% %numb_shortwins%
set perc_40mvs=%percent%
set /A engine_eas+=%percentx100%*100
set /A eas_singlestat_C_temp=%numb_shortwins%

REM *** Count number of wins of the engine, not longer than 45 moves ***
pgn-extract --quiet -bu%sh_level4% short_mvs_wins.pgn --output results.pgn > NUL
CALL :countgames results.pgn
set /A numb_shortwins=%c_numb_pgn%
set /A won_45_mvs=%numb_shortwins%
set /A numb_shortwins=%numb_shortwins%-%won_40_mvs%
CALL :percent %numb_wins% %numb_shortwins%
set perc_45mvs=%percent%
set /A engine_eas+=%percentx100%*68
set /A eas_singlestat_C_temp+=%numb_shortwins%

CALL :percent %numb_wins% %eas_singlestat_C_temp%
@echo %percent% %engine% >>eas_singlestat_C.txt

REM *** Count number of wins of the engine, not longer than 50 moves ***
pgn-extract --quiet -bu%sh_level3% short_mvs_wins.pgn --output results.pgn > NUL
CALL :countgames results.pgn
set /A numb_shortwins=%c_numb_pgn%
set /A won_50_mvs=%numb_shortwins%
set /A numb_shortwins=%numb_shortwins%-%won_45_mvs%
CALL :percent %numb_wins% %numb_shortwins%
set perc_50mvs=%percent%
set /A engine_eas+=%percentx100%*42

REM *** Count number of wins of the engine, not longer than 55 moves ***
pgn-extract --quiet -bu%sh_level2% short_mvs_wins.pgn --output results.pgn > NUL
CALL :countgames results.pgn
set /A numb_shortwins=%c_numb_pgn%
set /A won_55_mvs=%numb_shortwins%
set /A numb_shortwins=%numb_shortwins%-%won_50_mvs%
CALL :percent %numb_wins% %numb_shortwins%
set perc_55mvs=%percent%
set /A engine_eas+=%percentx100%*27

REM *** Count number of wins of the engine, not longer than 60 moves ***
CALL :countgames short_mvs_wins.pgn
set /A numb_shortwins=%c_numb_pgn%

REM *** For the ratinglist-output, build the percent-number of all short wins
CALL :percent %numb_wins% %numb_shortwins%
set percent_all_shorts=%percent%

@echo %percent% %engine% >>eas_singlestat_D.txt

set /A won_60_mvs=%numb_shortwins%
set /A numb_shortwins=%numb_shortwins%-%won_55_mvs%
CALL :percent %numb_wins% %numb_shortwins%
set perc_60mvs=%percent%
set /A engine_eas+=%percentx100%*18

set /A eas_short_wins=%engine_eas%-%eas_bad_draws%


REM *** Search for the won games with sacrifices, played by the engine ***

@echo|set /p="...1+ sacs white"
pgn-extract --quiet -y1_pawnsac_white whitewins.pgn --output results_opt1.pgn >NUL
del whitewins.pgn 2>nul
copy results_opt1.pgn whitewins.pgn > NUL
@echo|set /p="...1+ sacs black"
pgn-extract --quiet -y1_pawnsac_black blackwins.pgn --output blacksacs.pgn >NUL
REM *** Merge all found games in one single file ***
pgn-extract --quiet -aresults_opt1.pgn blacksacs.pgn >NUL

del blackwins.pgn 2>nul
copy blacksacs.pgn blackwins.pgn > NUL

REM ***************************************
REM *** NEW Early attack sacs detection ***
REM ***************************************

set /A numb_1sacs=0
CALL :countgames whitewins.pgn
set /A numb_1sacs=%c_numb_pgn%
CALL :countgames blackwins.pgn
set /A numb_1sacs=%c_numb_pgn%+%numb_1sacs%

REM *** Early sacs movelimit must be icreased by 8, because the sac-search needs 8 plies for finding a sac
set /A earlysearchsac_limit=%earlysac_limit%+8

REM *** Cut all sac games found so far at the lowest short-wins movenumber 
del whitewins_cutoff.pgn 2>nul
del blackwins_cutoff.pgn 2>nul
del earlysacs.pgn 2>nul
pgn-extract --quiet --plylimit %earlysearchsac_limit% whitewins.pgn --output whitewins_cutoff.pgn >NUL
pgn-extract --quiet --plylimit %earlysearchsac_limit% blackwins.pgn --output blackwins_cutoff.pgn >NUL

REM *** Now search again for sacs in the gamefiles with the cutted games
pgn-extract --quiet -y1_pawnsac_white whitewins_cutoff.pgn --output earlysacs.pgn >NUL
pgn-extract --quiet -y1_pawnsac_black blackwins_cutoff.pgn -aearlysacs.pgn >NUL

CALL :countgames earlysacs.pgn
set /A numb_early=%c_numb_pgn%

REM *** Build percents (how many sacs were played early in the game)
CALL :percent %numb_1sacs% %numb_early%

REM *** Calculate the earlysac bonus ***
set /A earlysacs_points=(%percentx100%/18)*(%percentx100%/18)
set earlysacs_percent=%percent%

@echo %percent% %engine% >>eas_singlestat_A.txt


REM *** Add EAS Bonuspoints if the average length of the won games by the engine is
REM *** lower than the average length of all won games in the source.pgn
set /A check_average=%avg_length_all_wins%-%avg_length_eng_wins%
if %check_average% GTR 0 set /A earlysacs_points+=(%check_average%*5000)

set /A engine_eas+=%earlysacs_points%
set /A eas_earlysacs=%earlysacs_points%


REM *******************************************************************************************
REM *** Continue normal sac-search 


@echo|set /p="...2-5+ sacs"
pgn-extract --quiet -y2_pawnsac_white whitewins.pgn --output results_opt2.pgn >NUL
pgn-extract --quiet -y2_pawnsac_black blackwins.pgn --output blacksacs.pgn >NUL
del whitewins.pgn 2>nul
del blackwins.pgn 2>nul
copy results_opt2.pgn whitewins.pgn > NUL
copy blacksacs.pgn blackwins.pgn > NUL
REM *** Merge all found games in one single file ***
pgn-extract --quiet -aresults_opt2.pgn blacksacs.pgn >NUL
fsutil file createnew blacksacs.pgn 0 >NUL

pgn-extract --quiet -y3_pawnsac_white whitewins.pgn --output results_opt3.pgn >NUL
pgn-extract --quiet -y3_pawnsac_black blackwins.pgn --output blacksacs.pgn >NUL
del whitewins.pgn 2>nul
del blackwins.pgn 2>nul
copy results_opt3.pgn whitewins.pgn > NUL
copy blacksacs.pgn blackwins.pgn > NUL
REM *** Merge all found games in one single file ***
pgn-extract --quiet -aresults_opt3.pgn blacksacs.pgn >NUL
fsutil file createnew blacksacs.pgn 0 >NUL

pgn-extract --quiet -y4_pawnsac_white whitewins.pgn --output results_opt4.pgn >NUL
pgn-extract --quiet -y4_pawnsac_black blackwins.pgn --output blacksacs.pgn >NUL
del whitewins.pgn 2>nul
del blackwins.pgn 2>nul
copy results_opt4.pgn whitewins.pgn > NUL
copy blacksacs.pgn blackwins.pgn > NUL
REM *** Merge all found games in one single file ***
pgn-extract --quiet -aresults_opt4.pgn blacksacs.pgn >NUL
fsutil file createnew blacksacs.pgn 0 >NUL

pgn-extract --quiet -y5_pawnsac_white whitewins.pgn --output results_opt5.pgn >NUL
pgn-extract --quiet -y5_pawnsac_black blackwins.pgn --output blacksacs.pgn >NUL
del whitewins.pgn 2>nul
del blackwins.pgn 2>nul
copy results_opt5.pgn whitewins.pgn > NUL
copy blacksacs.pgn blackwins.pgn > NUL
REM *** Merge all found games in one single file ***
pgn-extract --quiet -aresults_opt5.pgn blacksacs.pgn >NUL

pgn-extract --quiet -yqueensac_white whitewins.pgn --output results_opt9.pgn >NUL
pgn-extract --quiet -yqueensac_black blackwins.pgn --output blacksacs.pgn >NUL
REM *** Merge all found games in one single file ***
pgn-extract --quiet -aresults_opt9.pgn blacksacs.pgn >NUL

REM *** Delete games in the gamebases with lower sacs, which already occur in higher sacs files...***
pgn-extract --quiet -D results_opt9.pgn --output unique_opt9.pgn >NUL
pgn-extract --quiet -cresults_opt9.pgn -D -ounique_opt5.pgn results_opt5.pgn > NUL
pgn-extract --quiet -cresults_opt5.pgn -D -ounique_opt4.pgn results_opt4.pgn > NUL
pgn-extract --quiet -cresults_opt4.pgn -D -ounique_opt3.pgn results_opt3.pgn > NUL
pgn-extract --quiet -cresults_opt3.pgn -D -ounique_opt2.pgn results_opt2.pgn > NUL
pgn-extract --quiet -cresults_opt2.pgn -D -ounique_opt1.pgn results_opt1.pgn > NUL

REM *** Count the number of games in the result bases...***

CALL :countgames unique_opt9.pgn
set /A numb_opt9=%c_numb_pgn%
pgn-extract --quiet unique_opt9.pgn -acollect_sacgames_9.pgn > NUL
CALL :countgames unique_opt5.pgn
set /A numb_opt5=%c_numb_pgn%
pgn-extract --quiet unique_opt5.pgn -acollect_sacgames_5.pgn > NUL
CALL :countgames unique_opt4.pgn
set /A numb_opt4=%c_numb_pgn%
pgn-extract --quiet unique_opt4.pgn -acollect_sacgames_4.pgn > NUL
CALL :countgames unique_opt3.pgn
set /A numb_opt3=%c_numb_pgn%
pgn-extract --quiet unique_opt3.pgn -acollect_sacgames_3.pgn > NUL
CALL :countgames unique_opt2.pgn
set /A numb_opt2=%c_numb_pgn%
pgn-extract --quiet unique_opt2.pgn -acollect_sacgames_2.pgn > NUL
CALL :countgames unique_opt1.pgn
set /A numb_opt1=%c_numb_pgn%
pgn-extract --quiet unique_opt1.pgn -acollect_sacgames_1.pgn > NUL

REM *** Count the number off all sacs found (add all 6 sac-numbers) ***
set /A numb_sum=%numb_opt1%+%numb_opt2%+%numb_opt3%+%numb_opt4%+%numb_opt5%+%numb_opt9%

REM *** For the ratinglist-output, build the percent-number of all sac-games 
CALL :percent %numb_wins% %numb_sum%
set percent_all_sacs=%percent%

@echo %percent% %engine% >>eas_singlestat_B.txt

set /A eas_sacs=0
REM *** Count the sacs-percents of won games (higher sacs = bonus) for a engine-sac ratinglist ***
CALL :percent %numb_wins% %numb_opt1%
set perc_sac1=%percent%
set /A eas_sacs+=%percentx100%*10

CALL :percent %numb_wins% %numb_opt2%
set perc_sac2=%percent%
set /A eas_sacs+=%percentx100%*50

CALL :percent %numb_wins% %numb_opt3%
set perc_sac3=%percent%
set /A eas_sacs+=%percentx100%*100

CALL :percent %numb_wins% %numb_opt4%
set perc_sac4=%percent%
set /A eas_sacs+=%percentx100%*150

CALL :percent %numb_wins% %numb_opt5%
set perc_sac5=%percent%
set /A eas_sacs+=%percentx100%*200

CALL :percent %numb_wins% %numb_opt9%
set perc_sac9=%percent%
set /A eas_sacs+=%percentx100%*250


set /A engine_eas+=%eas_sacs%


REM *** For a valid sorting of the EAS-scores, they have to be formatted... ***
CALL :format_eas %engine_eas%
set engine_eas=%formatted%
CALL :format_eas %eas_bad_draws%
set eas_bad_draws=%formatted%
CALL :format_eas %eas_short_wins%
set eas_short_wins=%formatted%
CALL :format_eas %eas_sacs%
set eas_sacs=%formatted%
CALL :format_eas %eas_earlysacs%
set eas_earlysacs=%formatted%

REM *** Format number of wins
CALL :format_wins %numb_wins%
set winform=%formatted%

REM *** Format average length of won games by engine
CALL :format_engwins %avg_length_eng_wins%
set avg_length_eng_wins=%formatted%

REM *** Format average length of won games by engine
CALL :format_engwins %avg_length_all_wins%
set avg_length_all_wins=%formatted%

REM *** write the score and the engine-name in a file, but first remove the " " from the engine-name
for /f "useback tokens=*" %%a in ('%engine%') do set engine=%%~a

REM *** If overall number of wins/draws is too small, print a warning behind the engine name in the first ratinglist...
if %warning% GTR 0 @echo  %engine_eas%  %percent_all_sacs%  %earlysacs_percent%  %percent_all_shorts%  %percent_bad_draws%  %avg_length_eng_wins%   %engine%   XXXXX WARNING: Not enough games, EAS-score not reliable [50+ wins and 30+ draws needed] XXXXX >>statistics_EAS_rating_work.txt
if %warning% EQU 0 @echo  %engine_eas%  %percent_all_sacs%  %earlysacs_percent%  %percent_all_shorts%  %percent_bad_draws%  %avg_length_eng_wins%   %engine% >>statistics_EAS_rating_work.txt

REM *** 2nd list with more stats
@echo  %engine_eas%   %winform%  %avg_length_eng_wins%   %percent_all_sacs% =[%perc_sac9% + %perc_sac5% + %perc_sac4% + %perc_sac3% + %perc_sac2% + %perc_sac1%] %earlysacs_percent%   %percent_all_shorts% = [%perc_40mvs% + %perc_45mvs% + %perc_50mvs% + %perc_55mvs% + %perc_60mvs%]  %percent_bad_draws%   %engine% >>statistics_EAS_rating_work2.txt

REM *** 3rd list with EAS-points instead of percents
@echo  %engine_eas%    %eas_sacs%  %eas_earlysacs%  %eas_short_wins%  %eas_bad_draws%    %engine% >>statistics_EAS_rating_work3.txt

cd ../

REM *** All engines done? then exit, otherwise jump back but show estimated time to finish first ***
@echo ...done
@echo *********************************************************************************************
CALL :StopTimer2
CALL :EstimatedTime
@echo *********************************************************************************************

set /A engine_counter=%engine_counter%+1
if %engine_counter% GTR %numb_engines% goto exitloop02
cd ./bin
goto loop02
:exitloop02

@echo *********************************************************************************************
@echo *** Done! Building the EAS statistics file and the interesting_wins.pgn file...
@echo *********************************************************************************************

REM *** Build the EAS-ratinglist file ***
del statistics_EAS_ratinglist.txt 2>nul
@echo ***************************************************************************** >statistics_EAS_ratinglist.txt
@echo *** Engine Aggressiveness Tool V6.0 Score points Ratinglist >>statistics_EAS_ratinglist.txt
@echo ***************************************************************************** >>statistics_EAS_ratinglist.txt
@echo *** Meanwhile, the scoring-system of the EAS-Tool got really complex, so >>statistics_EAS_ratinglist.txt
@echo *** please check out the ReadMe-file, where you find the explanation... >>statistics_EAS_ratinglist.txt
@echo ***************************************************************************** >>statistics_EAS_ratinglist.txt
@echo *** Evaluated file: %gamebase% >>statistics_EAS_ratinglist.txt
@echo ***************************************************************************** >>statistics_EAS_ratinglist.txt
@echo                          early           bad  avg.win >>statistics_EAS_ratinglist.txt
@echo Rank  EAS-Score  sacs    sacs   shorts  draws  moves  Engine/player >>statistics_EAS_ratinglist.txt
@echo --------------------------------------------------------------------------->>statistics_EAS_ratinglist.txt
cd ./bin
del listnumbs.txt 2>nul
del listnumbs2.txt 2>nul
del list2numbs.txt 2>nul
del list2numbs2.txt 2>nul

REM *** Sorting lists by eas-score, then add rank-numbers to the EAS-ratinglist ***
sort /r statistics_EAS_rating_work.txt >>listnumbs.txt
sort /r statistics_EAS_rating_work2.txt >>list2numbs.txt
sort /r statistics_EAS_rating_work3.txt >>list3numbs.txt

set /A listnumber=0
:listloop01
set /A listnumber+=1
set /p nextengine=<listnumbs.txt
more +1 listnumbs.txt > listnumbs2.txt
del listnumbs.txt 2>nul
rename listnumbs2.txt listnumbs.txt

REM *** Format engine-number for output
CALL :format_engines %listnumber%
@echo %formatted%  %nextengine% >>../statistics_EAS_ratinglist.txt

if %listnumber% LSS %numb_engines% goto listloop01
@echo ------------------------------------------------------------------->>../statistics_EAS_ratinglist.txt
@echo *** Average length of all won games:            %avg_length_all_wins% moves>>../statistics_EAS_ratinglist.txt
@echo *** Movelimit for early sac bonus  :             %earlysac_limit% moves>>../statistics_EAS_ratinglist.txt

REM *** EAS single stats subroutine 
CALL :eas_single_stats
more eas_singlestat_out.txt >>../statistics_EAS_ratinglist.txt


@echo ***************************************************************************** >>../statistics_EAS_ratinglist.txt
@echo ***************************************************************************** >>../statistics_EAS_ratinglist.txt
@echo ***************************************************************************** >>../statistics_EAS_ratinglist.txt
@echo *** 2nd Ratinglist with more stats in percent-values ************************ >>../statistics_EAS_ratinglist.txt
@echo ***************************************************************************** >>../statistics_EAS_ratinglist.txt
@echo *** Average length of all won games                  :%avg_length_all_wins% moves >>../statistics_EAS_ratinglist.txt
@echo *** Calculated limit for short wins giving EAS-points: %shortwin_movelimit% moves >>../statistics_EAS_ratinglist.txt
@echo *** Movelimit for early sac bonus                    : %earlysac_limit% moves >>../statistics_EAS_ratinglist.txt

@echo ***************************************************************************** >>../statistics_EAS_ratinglist.txt
@echo                        avg.win                                                               early                                                            bad   >>../statistics_EAS_ratinglist.txt
@echo Rank  EAS-Score   wins  moves   sacs    sacsQ    sacs5+   sacs4    sacs3    sacs2    sacs1   sacs   all shorts short%sh_level5%  short%sh_level4%  short%sh_level3%  short%sh_level2%  short%sh_level1%   draws    Engine/player>>../statistics_EAS_ratinglist.txt
@echo ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------->>../statistics_EAS_ratinglist.txt

REM *** Build second ratinglist with all numbers of wins/sacs/short-wins ***
set /A listnumber=0
:list2loop01
set /A listnumber+=1
set /p nextengine=<list2numbs.txt
more +1 list2numbs.txt > list2numbs2.txt
del list2numbs.txt 2>nul
rename list2numbs2.txt list2numbs.txt

REM *** Format engine-number for output
CALL :format_engines %listnumber%
@echo %formatted%  %nextengine% >>../statistics_EAS_ratinglist.txt

if %listnumber% LSS %numb_engines% goto list2loop01


@echo ***************************************************************************** >>../statistics_EAS_ratinglist.txt
@echo ***************************************************************************** >>../statistics_EAS_ratinglist.txt
@echo ***************************************************************************** >>../statistics_EAS_ratinglist.txt
@echo *** 3rd Ratinglist, showing EAS-points instead of percents ****************** >>../statistics_EAS_ratinglist.txt
@echo ***************************************************************************** >>../statistics_EAS_ratinglist.txt
@echo *** (Mention, 5000 EAS-points are added to early sac bonus, for each move,*** >>../statistics_EAS_ratinglist.txt
@echo *** the average length of won games of the engine is shorter than the     *** >>../statistics_EAS_ratinglist.txt
@echo *** average length of all wins in the source.pgn.)                        *** >>../statistics_EAS_ratinglist.txt
@echo ***************************************************************************** >>../statistics_EAS_ratinglist.txt
@echo ***************************************************************************** >>../statistics_EAS_ratinglist.txt
@echo                              early              bad >>../statistics_EAS_ratinglist.txt
@echo Rank  EAS-Score      sacs     sacs   shorts    draws    Engine/player >>../statistics_EAS_ratinglist.txt
@echo ------------------------------------------------------------------------------------->>../statistics_EAS_ratinglist.txt


REM *** Build 3rd ratinglist with points instead of percents
set /A listnumber=0
:list3loop01
set /A listnumber+=1
set /p nextengine=<list3numbs.txt
more +1 list3numbs.txt > list3numbs3.txt
del list3numbs.txt 2>nul
rename list3numbs3.txt list3numbs.txt

REM *** Format engine-number for output
CALL :format_engines %listnumber%
@echo %formatted%  %nextengine% >>../statistics_EAS_ratinglist.txt

if %listnumber% LSS %numb_engines% goto list3loop01

del listnumbs.txt 2>nul
del listnumbs2.txt 2>nul
del list2numbs.txt 2>nul
del list2numbs2.txt 2>nul
del list3numbs.txt 2>nul
del list3numbs3.txt 2>nul
del statistics_EAS_rating_work.txt 2>nul
del statistics_EAS_rating_work2.txt 2>nul
del statistics_EAS_rating_work3.txt 2>nul

REM *** delete all working files ***

del whitewins_cutoff.pgn 2>nul
del blackwins_cutoff.pgn 2>nul
del earlysacs.pgn 2>nul

del results.pgn 2>nul
del results_opt1.pgn 2>nul
del results_opt2.pgn 2>nul 
del results_opt3.pgn 2>nul 
del results_opt4.pgn 2>nul 
del results_opt5.pgn 2>nul 
del results_opt9.pgn 2>nul 
del unique_opt1.pgn 2>nul 
del unique_opt2.pgn 2>nul 
del unique_opt3.pgn 2>nul 
del unique_opt4.pgn 2>nul 
del unique_opt5.pgn 2>nul 
del unique_opt9.pgn 2>nul 
del whitewins.pgn 2>nul
del blackwins.pgn 2>nul
del blacksacs.pgn 2>nul
del short_mvs_wins.pgn 2>nul
del bad_draws.pgn 2>nul
del bad_draws2.pgn 2>nul
del bad_draws_final.pgn 2>nul
del savedgames.pgn 2>nul
del enginegames.pgn 2>nul
del enginedraws.pgn 2>nul
del newsource.pgn 2>nul
del newsource_onlywins.pgn 2>nul
REM *** EAS single stats ***
del eas_singlestat_A.txt 2>nul
del eas_singlestat_B.txt 2>nul
del eas_singlestat_C.txt 2>nul
del eas_singlestat_D.txt 2>nul
del eas_singlestat_E.txt 2>nul
del eas_singlestat_F.txt 2>nul
del eas_singlestat_A_sorted.txt 2>nul
del eas_singlestat_B_sorted.txt 2>nul
del eas_singlestat_C_sorted.txt 2>nul
del eas_singlestat_D_sorted.txt 2>nul
del eas_singlestat_E_sorted.txt 2>nul
del eas_singlestat_F_sorted.txt 2>nul
del eas_singlestat_out.txt 2>nul


REM *** Sort final databases by length ***
CALL :countgames collect_sacgames_9.pgn
if %c_numb_pgn% GTR 0 (
  CALL :sortlength collect_sacgames_9.pgn
  del collect_sacgames_9.pgn 2>nul
  rename sortedlength.pgn collect_sacgames_9.pgn
)
CALL :countgames collect_sacgames_5.pgn
if %c_numb_pgn% GTR 0 (
  CALL :sortlength collect_sacgames_5.pgn
  del collect_sacgames_5.pgn 2>nul
  rename sortedlength.pgn collect_sacgames_5.pgn
)
CALL :countgames collect_sacgames_4.pgn
if %c_numb_pgn% GTR 0 (
  CALL :sortlength collect_sacgames_4.pgn
  del collect_sacgames_4.pgn 2>nul
  rename sortedlength.pgn collect_sacgames_4.pgn
)
CALL :countgames collect_sacgames_3.pgn
if %c_numb_pgn% GTR 0 (
  CALL :sortlength collect_sacgames_3.pgn
  del collect_sacgames_3.pgn 2>nul
  rename sortedlength.pgn collect_sacgames_3.pgn
)
CALL :countgames collect_sacgames_2.pgn
if %c_numb_pgn% GTR 0 (
  CALL :sortlength collect_sacgames_2.pgn
  del collect_sacgames_2.pgn 2>nul
  rename sortedlength.pgn collect_sacgames_2.pgn
)
CALL :countgames collect_sacgames_1.pgn
if %c_numb_pgn% GTR 0 (
  CALL :sortlength collect_sacgames_1.pgn
  del collect_sacgames_1.pgn 2>nul
  rename sortedlength.pgn collect_sacgames_1.pgn
)

REM *** Adding the Annotator-Tag with the Sac-information and merge found games in output-file
del out9.pgn 2>nul
fsutil file out9.pgn 0 >NUL

del newtag 2>nul
copy anno_9sac newtag > NUL
tagCreate newtag collect_sacgames_9.pgn  >NUL
pgn-extract --quiet out9.pgn --output foundgames.pgn > NUL

del newtag 2>nul
copy anno_5sac newtag > NUL
tagCreate newtag collect_sacgames_5.pgn  >NUL
pgn-extract --quiet out9.pgn -afoundgames.pgn > NUL

del newtag 2>nul
copy anno_4sac newtag > NUL
tagCreate newtag collect_sacgames_4.pgn  >NUL
pgn-extract --quiet out9.pgn -afoundgames.pgn > NUL

del newtag 2>nul
copy anno_3sac newtag > NUL
tagCreate newtag collect_sacgames_3.pgn  >NUL
pgn-extract --quiet out9.pgn -afoundgames.pgn > NUL

del newtag 2>nul
copy anno_2sac newtag > NUL
tagCreate newtag collect_sacgames_2.pgn  >NUL
pgn-extract --quiet out9.pgn -afoundgames.pgn > NUL

del newtag 2>nul
copy anno_1sac newtag > NUL
tagCreate newtag collect_sacgames_1.pgn  >NUL
pgn-extract --quiet out9.pgn -afoundgames.pgn > NUL

del out9.pgn 2>nul
fsutil file out9.pgn 0 >NUL

REM *** Sort final database by length ***
CALL :countgames collect_shorts.pgn
if %c_numb_pgn% GTR 0 (
  CALL :sortlength collect_shorts.pgn
  del collect_shorts.pgn 2>nul
  rename sortedlength.pgn collect_shorts.pgn
)

REM *** Adding the Annotator-Tag and merge found games in output-file
del newtag 2>nul
copy anno_very_short_game newtag > NUL
tagCreate newtag collect_shorts.pgn  >NUL
pgn-extract --quiet out9.pgn -afoundgames.pgn > NUL

del out9.pgn 2>nul
fsutil file out9.pgn 0 >NUL

pgn-extract --quiet -zno_endgame allwins.pgn --output results.pgn > NUL
pgn-extract --quiet -cresults.pgn -D -ono_endgame_wins.pgn allwins.pgn > NUL

REM *** Sort final database by length ***
CALL :countgames no_endgame_wins.pgn
if %c_numb_pgn% GTR 0 (
  CALL :sortlength no_endgame_wins.pgn
  del no_endgame_wins.pgn 2>nul
  rename sortedlength.pgn no_endgame_wins.pgn
)

REM *** Adding the Annotator-Tag with and merge found games in output-file
del newtag 2>nul
copy anno_before_endgame newtag > NUL
tagCreate newtag no_endgame_wins.pgn  >NUL
pgn-extract --quiet out9.pgn -afoundgames.pgn > NUL

del out9.pgn 2>nul
fsutil file out9.pgn 0 >NUL

pgn-extract --quiet -zimbalance allwins.pgn --output imbalance.pgn > NUL

REM *** Sort final database by length ***
CALL :countgames imbalance.pgn
if %c_numb_pgn% GTR 0 (
  CALL :sortlength imbalance.pgn
  del imbalance.pgn 2>nul
  rename sortedlength.pgn imbalance.pgn
)

REM *** Adding the Annotator-Tag with and merge found games in output-file
del newtag 2>nul
copy anno_material_imbalance newtag > NUL
tagCreate newtag imbalance.pgn >NUL
pgn-extract --quiet out9.pgn -afoundgames.pgn > NUL

pgn-extract --quiet -D foundgames.pgn --output ../interesting_wins.pgn >NUL
pgn-extract --quiet -D errorgames_collect.pgn --output ../errorgames.pgn >NUL

CALL :countgames foundgames.pgn
set /A numb_interesting=%c_numb_pgn%
CALL :countgames errorgames_collect.pgn
set /A numb_errors=%c_numb_pgn%

REM *** delete all working files ***
del errorgames.pgn 2>nul
del errorgames_collect.pgn  2>nul
del allwins.pgn 2>nul
del out9.pgn 2>nul
del newtag 2>nul
del results.pgn 2>nul
del results_opt1.pgn 2>nul
del results_opt2.pgn 2>nul 
del results_opt3.pgn 2>nul 
del results_opt4.pgn 2>nul 
del results_opt5.pgn 2>nul 
del results_opt9.pgn 2>nul 
del unique_opt1.pgn 2>nul 
del unique_opt2.pgn 2>nul 
del unique_opt3.pgn 2>nul 
del unique_opt4.pgn 2>nul 
del unique_opt5.pgn 2>nul 
del unique_opt9.pgn 2>nul 
del whitewins.pgn 2>nul
del blackwins.pgn 2>nul
del blacksacs.pgn 2>nul
del no_endgame_wins.pgn 2>nul
del imbalance.pgn 2>nul
del foundgames.pgn 2>nul
del newsource.pgn 2>nul
del sortedlength.pgn 2>nul
del collect_sacgames_1.pgn 2>nul
del collect_sacgames_2.pgn 2>nul
del collect_sacgames_3.pgn 2>nul
del collect_sacgames_4.pgn 2>nul
del collect_sacgames_5.pgn 2>nul
del collect_sacgames_9.pgn 2>nul
del collect_shorts.pgn 2>nul

cd ../

@echo ******************************************************************************************** >>statistics_EAS_ratinglist.txt
@echo ******************************************************************************************** >>statistics_EAS_ratinglist.txt
@echo **************************************************** >>statistics_EAS_ratinglist.txt
@echo *** EAS-Tool (C) 2025 Stefan Pohl (www.sp-cc.de) *** >>statistics_EAS_ratinglist.txt
@echo **************************************************** >>statistics_EAS_ratinglist.txt

@echo ****************
@echo *** Finished ***
@echo ****************
@echo *********************************************************************************************
@echo *** See the EAS Scores of the engines/players in the statistics_EAS_ratinglist.txt file
@echo *********************************************************************************************
@echo *** %numb_interesting% interesting wins are stored in the interesting_wins.pgn file 
@echo *** %numb_errors% games with non-regular endings are stored in the errorgames.pgn file
@echo *********************************************************************************************
CALL :StopTimer
CALL :DisplayTimerResult
@echo *********************************************************************************************
:error
CALL ./bin/playwav %windir%/media/alarm02.wav
pause
goto :EOF

:eas_single_stats
sort /r eas_singlestat_A.txt >>eas_singlestat_A_sorted.txt
sort /r eas_singlestat_B.txt >>eas_singlestat_B_sorted.txt
sort /r eas_singlestat_C.txt >>eas_singlestat_C_sorted.txt
sort /r eas_singlestat_D.txt >>eas_singlestat_D_sorted.txt
sort eas_singlestat_E.txt >>eas_singlestat_E_sorted.txt
sort eas_singlestat_F.txt >>eas_singlestat_F_sorted.txt

more eas_singlestat_A_sorted.txt > eas_gold_A_output.txt
more +1 eas_singlestat_A_sorted.txt > eas_silver_A_output.txt
more +2 eas_singlestat_A_sorted.txt > eas_bronze_A_output.txt
more +3 eas_singlestat_A_sorted.txt > eas_fourth_A_output.txt
more +4 eas_singlestat_A_sorted.txt > eas_fifth_A_output.txt
set /p eas_A_goldmedal=<eas_gold_A_output.txt
set /p eas_A_silvermedal=<eas_silver_A_output.txt
set /p eas_A_bronzemedal=<eas_bronze_A_output.txt
set /p eas_A_fourthmedal=<eas_fourth_A_output.txt
set /p eas_A_fifthmedal=<eas_fifth_A_output.txt
del eas_gold_A_output.txt 2>nul
del eas_silver_A_output.txt 2>nul
del eas_bronze_A_output.txt 2>nul
del eas_fourth_A_output.txt 2>nul
del eas_fifth_A_output.txt 2>nul
set eas_A_goldmedal=%eas_A_goldmedal:"=%
set eas_A_silvermedal=%eas_A_silvermedal:"=%
set eas_A_bronzemedal=%eas_A_bronzemedal:"=%
set eas_A_fourthmedal=%eas_A_fourthmedal:"=%
set eas_A_fifthmedal=%eas_A_fifthmedal:"=%

more eas_singlestat_B_sorted.txt > eas_gold_B_output.txt
more +1 eas_singlestat_B_sorted.txt > eas_silver_B_output.txt
more +2 eas_singlestat_B_sorted.txt > eas_bronze_B_output.txt
more +3 eas_singlestat_B_sorted.txt > eas_fourth_B_output.txt
more +4 eas_singlestat_B_sorted.txt > eas_fifth_B_output.txt
set /p eas_B_goldmedal=<eas_gold_B_output.txt
set /p eas_B_silvermedal=<eas_silver_B_output.txt
set /p eas_B_bronzemedal=<eas_bronze_B_output.txt
set /p eas_B_fourthmedal=<eas_fourth_B_output.txt
set /p eas_B_fifthmedal=<eas_fifth_B_output.txt
del eas_gold_B_output.txt 2>nul
del eas_silver_B_output.txt 2>nul
del eas_bronze_B_output.txt 2>nul
del eas_fourth_B_output.txt 2>nul
del eas_fifth_B_output.txt 2>nul
set eas_B_goldmedal=%eas_B_goldmedal:"=%
set eas_B_silvermedal=%eas_B_silvermedal:"=%
set eas_B_bronzemedal=%eas_B_bronzemedal:"=%
set eas_B_fourthmedal=%eas_B_fourthmedal:"=%
set eas_B_fifthmedal=%eas_B_fifthmedal:"=%

more eas_singlestat_C_sorted.txt > eas_gold_C_output.txt
more +1 eas_singlestat_C_sorted.txt > eas_silver_C_output.txt
more +2 eas_singlestat_C_sorted.txt > eas_bronze_C_output.txt
more +3 eas_singlestat_C_sorted.txt > eas_fourth_C_output.txt
more +4 eas_singlestat_C_sorted.txt > eas_fifth_C_output.txt
set /p eas_C_goldmedal=<eas_gold_C_output.txt
set /p eas_C_silvermedal=<eas_silver_C_output.txt
set /p eas_C_bronzemedal=<eas_bronze_C_output.txt
set /p eas_C_fourthmedal=<eas_fourth_C_output.txt
set /p eas_C_fifthmedal=<eas_fifth_C_output.txt
del eas_gold_C_output.txt 2>nul
del eas_silver_C_output.txt 2>nul
del eas_bronze_C_output.txt 2>nul
del eas_fourth_C_output.txt 2>nul
del eas_fifth_C_output.txt 2>nul
set eas_C_goldmedal=%eas_C_goldmedal:"=%
set eas_C_silvermedal=%eas_C_silvermedal:"=%
set eas_C_bronzemedal=%eas_C_bronzemedal:"=%
set eas_C_fourthmedal=%eas_C_fourthmedal:"=%
set eas_C_fifthmedal=%eas_C_fifthmedal:"=%

more eas_singlestat_D_sorted.txt > eas_gold_D_output.txt
more +1 eas_singlestat_D_sorted.txt > eas_silver_D_output.txt
more +2 eas_singlestat_D_sorted.txt > eas_bronze_D_output.txt
more +3 eas_singlestat_D_sorted.txt > eas_fourth_D_output.txt
more +4 eas_singlestat_D_sorted.txt > eas_fifth_D_output.txt
set /p eas_D_goldmedal=<eas_gold_D_output.txt
set /p eas_D_silvermedal=<eas_silver_D_output.txt
set /p eas_D_bronzemedal=<eas_bronze_D_output.txt
set /p eas_D_fourthmedal=<eas_fourth_D_output.txt
set /p eas_D_fifthmedal=<eas_fifth_D_output.txt
del eas_gold_D_output.txt 2>nul
del eas_silver_D_output.txt 2>nul
del eas_bronze_D_output.txt 2>nul
del eas_fourth_D_output.txt 2>nul
del eas_fifth_D_output.txt 2>nul
set eas_D_goldmedal=%eas_D_goldmedal:"=%
set eas_D_silvermedal=%eas_D_silvermedal:"=%
set eas_D_bronzemedal=%eas_D_bronzemedal:"=%
set eas_D_fourthmedal=%eas_D_fourthmedal:"=%
set eas_D_fifthmedal=%eas_D_fifthmedal:"=%

more eas_singlestat_E_sorted.txt > eas_gold_E_output.txt
more +1 eas_singlestat_E_sorted.txt > eas_silver_E_output.txt
more +2 eas_singlestat_E_sorted.txt > eas_bronze_E_output.txt
more +3 eas_singlestat_E_sorted.txt > eas_fourth_E_output.txt
more +4 eas_singlestat_E_sorted.txt > eas_fifth_E_output.txt
set /p eas_E_goldmedal=<eas_gold_E_output.txt
set /p eas_E_silvermedal=<eas_silver_E_output.txt
set /p eas_E_bronzemedal=<eas_bronze_E_output.txt
set /p eas_E_fourthmedal=<eas_fourth_E_output.txt
set /p eas_E_fifthmedal=<eas_fifth_E_output.txt
del eas_gold_E_output.txt 2>nul
del eas_silver_E_output.txt 2>nul
del eas_bronze_E_output.txt 2>nul
del eas_fourth_E_output.txt 2>nul
del eas_fifth_E_output.txt 2>nul
set eas_E_goldmedal=%eas_E_goldmedal:"=%
set eas_E_silvermedal=%eas_E_silvermedal:"=%
set eas_E_bronzemedal=%eas_E_bronzemedal:"=%
set eas_E_fourthmedal=%eas_E_fourthmedal:"=%
set eas_E_fifthmedal=%eas_E_fifthmedal:"=%

more eas_singlestat_F_sorted.txt > eas_gold_F_output.txt
more +1 eas_singlestat_F_sorted.txt > eas_silver_F_output.txt
more +2 eas_singlestat_F_sorted.txt > eas_bronze_F_output.txt
more +3 eas_singlestat_F_sorted.txt > eas_fourth_F_output.txt
more +4 eas_singlestat_F_sorted.txt > eas_fifth_F_output.txt
set /p eas_F_goldmedal=<eas_gold_F_output.txt
set /p eas_F_silvermedal=<eas_silver_F_output.txt
set /p eas_F_bronzemedal=<eas_bronze_F_output.txt
set /p eas_F_fourthmedal=<eas_fourth_F_output.txt
set /p eas_F_fifthmedal=<eas_fifth_F_output.txt
del eas_gold_F_output.txt 2>nul
del eas_silver_F_output.txt 2>nul
del eas_bronze_F_output.txt 2>nul
del eas_fourth_F_output.txt 2>nul
del eas_fifth_F_output.txt 2>nul
set eas_F_goldmedal=%eas_F_goldmedal:"=%
set eas_F_silvermedal=%eas_F_silvermedal:"=%
set eas_F_bronzemedal=%eas_F_bronzemedal:"=%
set eas_F_fourthmedal=%eas_F_fourthmedal:"=%
set eas_F_fifthmedal=%eas_F_fifthmedal:"=%

@echo ********************************************************************************************* >eas_singlestat_out.txt
@echo ********************************************************************************************* >>eas_singlestat_out.txt
@echo ********************************************************************************************* >>eas_singlestat_out.txt
@echo *** EAS single-statistics (6 categories, each with Top5 engines): >>eas_singlestat_out.txt
@echo ********************************************************************************************* >>eas_singlestat_out.txt
@echo A: Early sacrifices (percents of all sacs)           : [1]:%eas_A_goldmedal% >>eas_singlestat_out.txt
@echo                                                        [2]:%eas_A_silvermedal% >>eas_singlestat_out.txt
@echo                                                        [3]:%eas_A_bronzemedal% >>eas_singlestat_out.txt
@echo                                                        [4]:%eas_A_fourthmedal% >>eas_singlestat_out.txt
@echo                                                        [5]:%eas_A_fifthmedal% >>eas_singlestat_out.txt
@echo B: Most sacrifices overall                           : [1]:%eas_B_goldmedal% >>eas_singlestat_out.txt
@echo                                                        [2]:%eas_B_silvermedal% >>eas_singlestat_out.txt
@echo                                                        [3]:%eas_B_bronzemedal% >>eas_singlestat_out.txt
@echo                                                        [4]:%eas_B_fourthmedal% >>eas_singlestat_out.txt
@echo                                                        [5]:%eas_B_fifthmedal% >>eas_singlestat_out.txt
@echo C: Very short wins (%sh_level4% moves or less)                : [1]:%eas_C_goldmedal% >>eas_singlestat_out.txt
@echo                                                        [2]:%eas_C_silvermedal% >>eas_singlestat_out.txt
@echo                                                        [3]:%eas_C_bronzemedal% >>eas_singlestat_out.txt
@echo                                                        [4]:%eas_C_fourthmedal% >>eas_singlestat_out.txt
@echo                                                        [5]:%eas_C_fifthmedal% >>eas_singlestat_out.txt
@echo D: Most short wins overall                           : [1]:%eas_D_goldmedal% >>eas_singlestat_out.txt
@echo                                                        [2]:%eas_D_silvermedal% >>eas_singlestat_out.txt
@echo                                                        [3]:%eas_D_bronzemedal% >>eas_singlestat_out.txt
@echo                                                        [4]:%eas_D_fourthmedal% >>eas_singlestat_out.txt
@echo                                                        [5]:%eas_D_fifthmedal% >>eas_singlestat_out.txt
@echo E: Average length of all won games                   : [1]:%eas_E_goldmedal% >>eas_singlestat_out.txt
@echo                                                        [2]:%eas_E_silvermedal% >>eas_singlestat_out.txt
@echo                                                        [3]:%eas_E_bronzemedal% >>eas_singlestat_out.txt
@echo                                                        [4]:%eas_E_fourthmedal% >>eas_singlestat_out.txt
@echo                                                        [5]:%eas_E_fifthmedal% >>eas_singlestat_out.txt
@echo F: Smallest number of bad draws                      : [1]:%eas_F_goldmedal% >>eas_singlestat_out.txt
@echo                                                        [2]:%eas_F_silvermedal% >>eas_singlestat_out.txt
@echo                                                        [3]:%eas_F_bronzemedal% >>eas_singlestat_out.txt
@echo                                                        [4]:%eas_F_fourthmedal% >>eas_singlestat_out.txt
@echo                                                        [5]:%eas_F_fifthmedal% >>eas_singlestat_out.txt
@echo ********************************************************************************************* >>eas_singlestat_out.txt
@echo ********************************************************************************************* >>eas_singlestat_out.txt
exit /B 0

:countgames
set /A c_numb_pgn=0
for /f "tokens=2,* delims= " %%G in ('find /C "[White " %~1') do set /A c_numb_pgn=%%H
exit /B 0

:moveaverage
set ma_gamefile=%~1
CALL :countgames %ma_gamefile%
if %c_numb_pgn% LEQ 0 (
set /A ma_moveaverage=0
EXIT /B 0
)
summary.exe %ma_gamefile% >NUL
for /f "tokens=1-10 delims=., " %%A in ('find "Average" outSummary') do ( 
set ma_avg=%%I >NUL
set ma_rest=%%J > NUL
)
set ma_avg=%ma_avg:~0,-1%
set ma_rest=%ma_rest:~0,-1%
REM *** Round ply-moveaverage up, if necessary
if %ma_rest% GEQ 50 set /A ma_avg+=1
set /A ma_moveaverage=%ma_avg%/2
del outSummary 2>nul
EXIT /B 0

:percent
if %~1 LEQ 0 (
set percent=00.00%%
set /A percentx100=0
EXIT /B 0
)
set /A l_base=((1000000000/%~1)*%~2)
set /A l_percent=%l_base%/1000000
set /A l_rest1=%l_percent%%%10
set /A l_percent=%l_percent%/10
set /A l_secdig_pc=%l_base%/100000
set /A l_rest2=%l_secdig_pc%%%10
set /A l_thirddig_pc=%l_base%/10000
set /A l_rest3=%l_thirddig_pc%%%10

REM *** Round 2nd digit up, if 3rd digit is GEQ 5 and round 1st digit and percent-value up, if necessary
if %l_rest3% GEQ 5 set /A l_rest2+=1
if %l_rest2% GEQ 10 (
  set /A l_rest2-=10
  set /A l_rest1+=1
)
if %l_rest1% GEQ 10 (
  set /A l_rest1-=10
  set /A l_percent+=1
)

if %l_percent% LSS 10 (
  set percent=0%l_percent%.%l_rest1%%l_rest2%%%
) else (
  set percent=%l_percent%.%l_rest1%%l_rest2%%%
)
set /A percentx100=(%l_percent%*100)+(%l_rest1%*10)+%l_rest2%

if %l_percent% GTR 99 (
set percent=100.0%%
set /A percentx100=10000
)
EXIT /B 0

:format_eas
set /A fn_numb=%~1
if %fn_numb% LEQ 9 (
set formatted=      %fn_numb%
goto exitformat_eas)
if %fn_numb% LEQ 99 (
set formatted=     %fn_numb%
goto exitformat_eas)
if %fn_numb% LEQ 999 (
set formatted=    %fn_numb%
goto exitformat_eas)
if %fn_numb% LEQ 9999 (
set formatted=   %fn_numb%
goto exitformat_eas)
if %fn_numb% LEQ 99999 (
set formatted=  %fn_numb%
goto exitformat_eas)
if %fn_numb% LEQ 999999 (
set formatted= %fn_numb%
goto exitformat_eas)
if %fn_numb% LEQ 9999999 (
set formatted=%fn_numb%
goto exitformat_eas)
:exitformat_eas
EXIT /B 0

:format_wins
set /A fn_numb=%~1
if %fn_numb% LEQ 9 (
set formatted=     %fn_numb%
goto exitformat_wins)
if %fn_numb% LEQ 99 (
set formatted=    %fn_numb%
goto exitformat_wins)
if %fn_numb% LEQ 999 (
set formatted=   %fn_numb%
goto exitformat_wins)
if %fn_numb% LEQ 9999 (
set formatted=  %fn_numb%
goto exitformat_wins)
if %fn_numb% LEQ 99999 (
set formatted= %fn_numb%
goto exitformat_wins)
if %fn_numb% LEQ 999999 (
set formatted=%fn_numb%
goto exitformat_eas)
:exitformat_wins
EXIT /B 0

:format_engines
set /A fn_numb=%~1
if %fn_numb% LEQ 9 (
set formatted=   %fn_numb%
goto exitformat_engines)
if %fn_numb% LEQ 99 (
set formatted=  %fn_numb%
goto exitformat_engines)
if %fn_numb% LEQ 999 (
set formatted= %fn_numb%
goto exitformat_engines)
if %fn_numb% LEQ 9999 (
set formatted=%fn_numb%
goto exitformat_engines)
:exitformat_engines
EXIT /B 0

:format_engwins
set /A fn_numb=%~1
if %fn_numb% LEQ 9 (
set formatted=  %fn_numb%
goto exitformat_engwins)
if %fn_numb% LEQ 99 (
set formatted= %fn_numb%
goto exitformat_engwins)
if %fn_numb% LEQ 999 set formatted=%fn_numb%
:exitformat_engwins
EXIT /B 0

:StartTimer
set StartTIME=%TIME%
for /f "usebackq tokens=1-4 delims=:., " %%f in (`echo %StartTIME: =0%`) do set /a Start100S=1%%f*360000+1%%g*6000+1%%h*100+1%%i-36610100
EXIT /B 0

:StopTimer
set StopTIME=%TIME%
for /f "usebackq tokens=1-4 delims=:., " %%f in (`echo %StopTIME: =0%`) do set /a Stop100S=1%%f*360000+1%%g*6000+1%%h*100+1%%i-36610100
if %Stop100S% LSS %Start100S% set /a Stop100S+=8640000
set /a TookTime=%Stop100S%-%Start100S%
set TookTimePadded=0%TookTime%
EXIT /B 0

:StartTimer2
set StartTIME2=%TIME%
for /f "usebackq tokens=1-4 delims=:., " %%f in (`echo %StartTIME2: =0%`) do set /a Start100S2=1%%f*360000+1%%g*6000+1%%h*100+1%%i-36610100
EXIT /B 0

:StopTimer2
set StopTIME2=%TIME%
for /f "usebackq tokens=1-4 delims=:., " %%f in (`echo %StopTIME2: =0%`) do set /a Stop100S2=1%%f*360000+1%%g*6000+1%%h*100+1%%i-36610100
if %Stop100S2% LSS %Start100S2% set /a Stop100S2+=8640000
set /a TookTime2=%Stop100S2%-%Start100S2%
set TookTimePadded2=0%TookTime2%
EXIT /B 0

:DisplayTimerResult
set /A t_elapsed=%TookTime:~0,-2%.%TookTimePadded:~-2% 2>nul
set /A t_seconds=%t_elapsed%
set /A t_minutes=%t_seconds%/60
set /A t_hours=%t_minutes%/60
set /A t_seconds-=%t_minutes%*60
set /A t_minutes-=%t_hours%*60
if %t_seconds% LEQ 9 set t_seconds=0%t_seconds%
if %t_minutes% LEQ 9 set t_minutes=0%t_minutes%
if %t_hours% LEQ 9 set t_hours=0%t_hours%
@echo *** Elapsed time: %t_hours%:%t_minutes%:%t_seconds%
EXIT /B 0

:EstimatedTime
set /A t_elapsed=%TookTime2:~0,-2%.%TookTimePadded2:~-2% 2>nul
set /A t_time_per_engine=%t_elapsed%/%engine_counter
set /A t_engines_to_do=(%numb_engines%-%engine_counter%)+1
set /A t_estimated=%t_engines_to_do%*%t_time_per_engine%
set /A t_seconds=%t_estimated%
set /A t_minutes=%t_seconds%/60
set /A t_hours=%t_minutes%/60
set /A t_seconds-=%t_minutes%*60
set /A t_minutes-=%t_hours%*60
if %t_seconds% LEQ 9 set t_seconds=0%t_seconds%
if %t_minutes% LEQ 9 set t_minutes=0%t_minutes%
if %t_hours% LEQ 9 set t_hours=0%t_hours%
@echo *** Estimated time until finishing: %t_hours%:%t_minutes%:%t_seconds%
EXIT /B 0

:sortlength
del sortedlength.pgn 2>nul
pgn-extract --quiet -bu19 %~1 --output sortedlength.pgn > NUL
pgn-extract --quiet -bu24 -bl20 %~1 -asortedlength.pgn > NUL
pgn-extract --quiet -bu29 -bl25 %~1 -asortedlength.pgn > NUL
pgn-extract --quiet -bu34 -bl30 %~1 -asortedlength.pgn > NUL
pgn-extract --quiet -bu39 -bl35 %~1 -asortedlength.pgn > NUL
pgn-extract --quiet -bu44 -bl40 %~1 -asortedlength.pgn > NUL
pgn-extract --quiet -bu49 -bl45 %~1 -asortedlength.pgn > NUL
pgn-extract --quiet -bu54 -bl50 %~1 -asortedlength.pgn > NUL
pgn-extract --quiet -bu59 -bl55 %~1 -asortedlength.pgn > NUL
pgn-extract --quiet -bu64 -bl60 %~1 -asortedlength.pgn > NUL
pgn-extract --quiet -bu69 -bl65 %~1 -asortedlength.pgn > NUL
pgn-extract --quiet -bu79 -bl70 %~1 -asortedlength.pgn > NUL
pgn-extract --quiet -bu89 -bl80 %~1 -asortedlength.pgn > NUL
pgn-extract --quiet -bu99 -bl90 %~1 -asortedlength.pgn > NUL
pgn-extract --quiet -bu109 -bl100 %~1 -asortedlength.pgn > NUL
pgn-extract --quiet -bu119 -bl110 %~1 -asortedlength.pgn > NUL
pgn-extract --quiet -bl120 %~1 -asortedlength.pgn > NUL
exit /B 0

:find_gauntlet
del tempgauntlet 2>nul
more +3 outName3 > tempgauntlet
set /p firstline=<tempgauntlet
set str=%firstline:~0,-8%
set str=%str%##
set str=%str:                ##=##%
set str=%str:        ##=##%
set str=%str:    ##=##%
set str=%str:  ##=##%
set str=%str: ##=##%
set str=%str:##=%
set engine="%str%"
del tempgauntlet 2>nul
del outName* 2>nul
EXIT /B 0
