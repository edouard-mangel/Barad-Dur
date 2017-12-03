import os
import subprocess
import credentials
from datetime import datetime
import re

# IP Cameras related variables
subDomain = '192.168.1.'
port = 543
fileName = 'sample_videoSalon3.avi'
ffmpegPath = '/home/edouard/Softs/ffmpeg/ffmpeg'
ffmpegArgs = ' -vcodec copy -acodec copy '
camerasIPsuffix = [19, 20, 21, 22]

# Video player related stuff
player = "omxplayer"
recordFolder = "./Videos/"

# Threads related processes
recording = None
playing = None


def GetIPAddress(cameraID):
    strAddress = '192.168.1.'
    if(0 <= cameraID <= 4):
        strAddress += str(camerasIPsuffix[cameraID])
    else:
        raise Exception("Argument out of bounds")
    return strAddress


def SetFileName(extension="avi"):
    global fileName
    fileName = str(datetime.date(datetime.now())) + \
        str(datetime.time(datetime.now()))
    fileName += "." + extension
    fileName = re.sub('[:]', '_', fileName)
    return fileName


def GetRecordCommand(cameraID, override=False):
    try:
        ipAddress = GetIPAddress(cameraID)
    except Exception as e:
        raise e
    command = ffmpegPath + ' -i rtsp://' + credentials.login() + ipAddress + \
        ffmpegArgs + recordFolder + SetFileName()
    if override == True:
        command += " -y"
    return command


def GetPlayCommand(fileName, loop=True):
    command = str.join(' ', (player, recordFolder + fileName))
    if loop == True:
        command += " --loop"
    print(command)
    return command


def ToggleRecord(cameraID):
    global recording
    global playing
    if recording is None:
        try:
            command = GetRecordCommand(cameraID, True)
            
            recording = subprocess.Popen(command.split(" ") )
        except Exception as e:
            raise e
        finally:
            playing = None
    else:
        recording.send_signal(2)
        recording = None
        playing = None
        return


def PlayVideo(fileName):
    command = GetPlayCommand(fileName)
    global playing
    if playing is None:
        try:
            playing = subprocess.Popen(command.split(" "))
        except Exception as e:
            raise e
    else:
        playing.send_signal(2)
        playing = None
        time.sleep(2)
        return
