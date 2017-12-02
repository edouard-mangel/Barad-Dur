import os
import subprocess
import credentials

# IP Cameras related variables
subDomain = '192.168.1.'
port = 543
fileName = 'sample_videoSalon3.avi'
ffmpegPath = '/home/edouard/Softs/ffmpeg/ffmpeg'
ffmpegArgs = ' -vcodec copy -acodec copy '
camerasIPsuffix = [19,20,21,22]

# Video player related stuff
player = "omxplayer"
playerOptions = "-o local"

def GetIPAddress(cameraID):
	strAddress = '192.168.1.'
	if( 0 <= cameraID <= 4 ):
		strAddress += str(camerasIPsuffix[cameraID])
	else: 
		raise Exception("Argument out of bounds")
	return strAddress

def GetRecordCommand(cameraID):
	try:
		ipAddress = GetIPAddress(cameraID)
	except Exception as e:
		raise e
	return ffmpegPath + ' -i rtsp://' + credentials.login() + ipAddress + ffmpegArgs + fileName

def GetPlayCommand(fileName, loop = True):
	command = str.join( ' ', (player, playerOptions, fileName))
	if loop == True:
		command += " --loop"
	return command

