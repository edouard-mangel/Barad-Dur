import os
import subprocess
import credentials

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


def SetFileName():
    global fileName
    return fileName


def GetRecordCommand(cameraID, override=False):
    try:
        ipAddress = GetIPAddress(cameraID)
    except Exception as e:
        raise e
    command = ffmpegPath + ' -i rtsp://' + credentials.login() + ipAddress + \
        ffmpegArgs + recordFolder + fileName
    if override == True:
        command += " -y"
    return command


def GetPlayCommand(fileName, loop=True):
    command = str.join(' ', (player, recordFolder + fileName))
    if loop == True:
        command += " --loop"
    return command


def ToggleRecord(cameraID):
    try:
        command = GetRecordCommand(cameraID, True)
    except Exception as e:
        raise e

    global recording
    global playing
    if recording is None:
        recording = subprocess.Popen(command.split(" "))
        playing = None
    else:
        recording.send_signal(2)
        recording = None
        playing = None
        return


def PlayVideo(fileName):
    print("\nTestPrint: " + fileName )
    command = GetPlayCommand(fileName)
    print("Commande de lecture: "+ command)
    global playing
    if playing is None:
    	try:
        	print("Command = " + command)
        	playing = subprocess.Popen(command.split(" "))
    	except Exception as e:
    		raise e
    else:
        playing.send_signal(2)
        playing = None
