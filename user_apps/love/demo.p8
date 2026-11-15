pico-8 cartridge // http://www.pico-8.com
version 42
__lua__
-- Star Run: original Tufty demo. Up/down steer, A dashes, B restarts.
function _init()
 lane=2
 ship_y=60
 ticks=0
 score=0
 shield=0
 cooldown=0
 started=false
 crashed=false
 rocks={}
 srand(2350)
 local sprite={"000c0000","00cc0000","0ccc7000","cccc7770",
               "0ccc7000","00cc0000","000c0000","00000000"}
 for row=1,8 do
  for column=1,8 do
   sset(column+7,row-1,tonum("0x"..sub(sprite[row],column,column)))
  end
 end
 sset(16,0,5)
 sset(20,4,1)
 for row=0,15 do
  for column=0,15 do mset(column,row,2) end
 end
 fset(2,1)
end

function _update()
 ticks=ticks+1
 if btnp(5) then _init() return end
 if not started then
  if btnp(4) then started=true end
  return
 end
 if crashed then return end
 if btnp(2) then lane=max(1,lane-1) end
 if btnp(3) then lane=min(3,lane+1) end
 ship_y=ship_y+(lane*30-ship_y)*0.4
 cooldown=max(0,cooldown-1)
 shield=max(0,shield-1)
 if btnp(4) and cooldown==0 then shield=18 cooldown=75 end
 if ticks%24==0 then
  add(rocks,{x=132,y=(flr(rnd(3))+1)*30,gem=rnd()<0.3})
 end
 for rock in all(rocks) do
  rock.x=rock.x-2-score/500
  if abs(rock.x-18)<8 and abs(rock.y-ship_y)<8 then
   if rock.gem or shield>0 then score=score+10 del(rocks,rock)
   else crashed=true end
  elseif rock.x< -8 then score=score+1 del(rocks,rock) end
 end
end

function _draw()
 cls(0)
 map(0,0,0,0,16,16,1)
 for lane_index=1,3 do
  line(0,lane_index*30+10,127,lane_index*30+10,1)
 end
 for index=1,12 do
  local star_x=(index*23-ticks*(index%3+1))%128
  pset(star_x,(index*37)%104+10,5)
 end
 for rock in all(rocks) do
  if rock.gem then
   circfill(rock.x,rock.y,3,10)
   pset(rock.x-1,rock.y-1,7)
  else
   circfill(rock.x,rock.y,5,8)
   circfill(rock.x+1,rock.y-1,2,2)
  end
 end
 if shield>0 then circ(18,ship_y,10,12) end
 line(9,ship_y,5-ticks%4,ship_y,9)
 spr(1,14,ship_y-3)
 rectfill(0,0,127,9,1)
 print("STAR RUN",3,2,12)
 print("SCORE "..score,76,2,7)
 rectfill(0,112,127,127,1)
 print("UP/DOWN STEER  A DASH",5,115,6)
 rectfill(5,123,5+(75-cooldown)*1.5,125,12)
 if not started or crashed then
  rectfill(8,42,119,85,0)
  rect(8,42,119,85,12)
  if crashed then
   print("SIGNAL LOST",41,50,8)
   print("B: TRY AGAIN",39,66,7)
  else
   print("S T A R  R U N",36,49,12)
   print("GOLD: COLLECT / RED: DODGE",16,61,6)
   print("A: LAUNCH",46,74,10)
  end
 end
end
