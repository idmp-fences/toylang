@echo off
setlocal enabledelayedexpansion

:: Check if the correct number of arguments was given
if "%~2"=="" (
    echo Usage: %~nx0 ^<filename^> ^<mode^>
    echo ^<mode^> can be 'ILP', 'ALNS', or 'BOTH'
    exit /b 1
)

:: Assign arguments to variables
set "FILENAME=%~1"
set "MODE=%~2"
set "OUTPUT_FILE=cycles.json"
set "CSV_OUTPUT=output.csv"
set "TEMP_FILE=temp.csv"

:: Measure time for cargo command using %time%
set _startCargo=%time%
cargo run -p toy find-cycles "%FILENAME%" > "%OUTPUT_FILE%"
set _endCargo=%time%

:: Convert start and end times to milliseconds since midnight
for /f "tokens=1-4 delims=:,." %%a in ("%_startCargo%") do (
    set /A "_startCargoMS=%%a*3600000 + 1%%b%%100*60000 + 1%%c%%1000 + 1%%d%%100"
)
for /f "tokens=1-4 delims=:,." %%a in ("%_endCargo%") do (
    set /A "_endCargoMS=%%a*3600000 + 1%%b%%100*60000 + 1%%c%%1000 + 1%%d%%100"
)

:: Calculate elapsed time for cargo command
set /A "_elapsedCargoMS=_endCargoMS-_startCargoMS"

:: Handle crossing midnight
if %_elapsedCargoMS% LSS 0 (
    set /A "_elapsedCargoMS=0-elapsedCargoMS"
)

:: Convert milliseconds to seconds
set /A "_elapsedCargoSec=_elapsedCargoMS/1000"
set /A "_elapsedCargoMS=_elapsedCargoMS%%1000"

:: Check if the CSV file exists and contains the filename and mode
set "FOUND_ILP=0"
set "FOUND_ALNS=0"
if exist "%CSV_OUTPUT%" (
    for /f "tokens=1-5 delims=," %%a in ('findstr /c:"%FILENAME%,ILP" "%CSV_OUTPUT%"') do (
        if "%%a"=="%FILENAME%" if "%%b"=="ILP" if "%MODE%"=="ILP" (
            for /f "usebackq tokens=*" %%a in ("%CSV_OUTPUT%") do (
                echo %%a | findstr /v /c:"%FILENAME%,ILP" >> "%TEMP_FILE%"
            )
            move /y "%TEMP_FILE%" "%CSV_OUTPUT%"
        ) else if "%%a"=="%FILENAME%" if "%%b"=="ILP" if "%MODE%"=="BOTH" (
            for /f "usebackq tokens=*" %%a in ("%CSV_OUTPUT%") do (
                echo %%a | findstr /v /c:"%FILENAME%,ILP" >> "%TEMP_FILE%"
            )
            move /y "%TEMP_FILE%" "%CSV_OUTPUT%"
        )
    )
    for /f "tokens=1-5 delims=," %%a in ('findstr /c:"%FILENAME%,ALNS" "%CSV_OUTPUT%"') do (
        if "%%a"=="%FILENAME%" if "%%b"=="ALNS" if "%MODE%"=="ALNS" (
            for /f "usebackq tokens=*" %%a in ("%CSV_OUTPUT%") do (
                echo %%a | findstr /v /c:"%FILENAME%,ALNS" >> "%TEMP_FILE%"
            )
            move /y "%TEMP_FILE%" "%CSV_OUTPUT%"
        ) else if "%%a"=="%FILENAME%" if "%%b"=="ALNS" if "%MODE%"=="BOTH" (
            for /f "usebackq tokens=*" %%a in ("%CSV_OUTPUT%") do (
                echo %%a | findstr /v /c:"%FILENAME%,ALNS" >> "%TEMP_FILE%"
            )
            move /y "%TEMP_FILE%" "%CSV_OUTPUT%"
        )
    )
)

if not exist "%CSV_OUTPUT%" echo Filename,Mode,CargoTime (s),ExperimentTime (s),Fences> "%CSV_OUTPUT%"
if "%MODE%"=="ILP" (
    echo %FILENAME%,ILP,!_elapsedCargoSec!.!_elapsedCargoMS!,, >> "%CSV_OUTPUT%"
)
if "%MODE%"=="ALNS" (
    echo %FILENAME%,ALNS,!_elapsedCargoSec!.!_elapsedCargoMS!,, >> "%CSV_OUTPUT%"
)
if "%MODE%"=="BOTH" (
    echo %FILENAME%,ILP,!_elapsedCargoSec!.!_elapsedCargoMS!,, >> "%CSV_OUTPUT%"
    echo %FILENAME%,ALNS,!_elapsedCargoSec!.!_elapsedCargoMS!,, >> "%CSV_OUTPUT%"
)

:: Check the mode and run corresponding command
if /I "%MODE%"=="ILP" (
    python experiment.py 1 "%FILENAME%" "%OUTPUT_FILE%" "%CSV_OUTPUT%"
) else if /I "%MODE%"=="ALNS" (
    python experiment.py 2 "%FILENAME%" "%OUTPUT_FILE%" "%CSV_OUTPUT%"
) else if /I "%MODE%"=="BOTH" (
    python experiment.py 1 "%FILENAME%" "%OUTPUT_FILE%" "%CSV_OUTPUT%"
    python experiment.py 2 "%FILENAME%" "%OUTPUT_FILE%" "%CSV_OUTPUT%"
) else (
    echo Invalid mode: %MODE%
    echo Please choose 'ILP', 'ALNS', or 'BOTH'
    exit /b 1
)

endlocal
