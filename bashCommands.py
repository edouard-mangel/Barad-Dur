import os
import subprocess
import credentials

subDomain = '192.168.1.'
port = 543
fileName = 'sample_videoSalon3.avi'
ffmpegPath = '/home/edouard/Softs/ffmpeg/ffmpeg'
ffmpegArgs = ' -vcodec copy -acodec copy '

camerasIPs = [19,20,21,22]

def GetIPAddress(cameraID):
	strAddress = '192.168.1.'
	if( 0 <= cameraID <= 4 ):
		strAddress += str(camerasIPs[cameraID])
	else: 
		raise Exception("Argument out of bounds")
	return strAddress

def GetCommand(cameraID):
	try:
		ipAddress = GetIPAddress(cameraID)
	except Exception as e:
		raise e
	return ffmpegPath + ' -i rtsp://' + credentials.login() + ipAddress + ffmpegArgs + fileName
		