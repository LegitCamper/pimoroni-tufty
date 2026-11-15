local player = 120
local obstacles = {}
local elapsed, score, spawn = 0, 0, 0
local playing = false

local function reset()
    player, elapsed, score, spawn = 120, 0, 0, 0
    obstacles = {}
    playing = true
end

function love.load()
    love.graphics.setBackgroundColor(0.025, 0.055, 0.09)
    math.randomseed(2350)
end

function love.keypressed(key)
    if key == "z" or key == "return" then reset() end
end

function love.update(delta)
    elapsed = elapsed + delta
    if not playing then return end
    score = score + delta * 10
    if love.keyboard.isDown("up") then player = player - delta * 145 end
    if love.keyboard.isDown("down") then player = player + delta * 145 end
    player = math.max(45, math.min(201, player))
    spawn = spawn - delta
    if spawn <= 0 then
        obstacles[#obstacles + 1] = {left = 320, top = math.random(45, 193)}
        spawn = 0.65
    end
    for index = #obstacles, 1, -1 do
        local obstacle = obstacles[index]
        obstacle.left = obstacle.left - delta * (95 + math.min(score, 120))
        if math.abs(obstacle.left - 53) < 13 and math.abs(obstacle.top - player) < 13 then
            playing = false
        end
        if obstacle.left < -20 then table.remove(obstacles, index) end
    end
end

function love.draw()
    love.graphics.setColor(0.08, 0.17, 0.22)
    for column = 0, 10 do
        local left = column * 32 - (elapsed * 25) % 32
        love.graphics.line(left, 32, left, 212)
    end
    for row = 1, 6 do love.graphics.line(0, 32 + row * 30, 319, 32 + row * 30) end
    love.graphics.setColor(0.35, 1, 0.72)
    love.graphics.print("ORBIT DODGE", 12, 10)
    love.graphics.print(string.format("%05d", math.floor(score)), 270, 10)
    love.graphics.line(0, 30, 319, 30)
    love.graphics.rectangle("fill", 43, player - 6, 19, 12)
    love.graphics.setColor(1, 0.68, 0.25)
    love.graphics.rectangle("fill", 36, player - 3, 5, 6)
    love.graphics.setColor(1, 0.32, 0.38)
    for _, obstacle in ipairs(obstacles) do
        love.graphics.rectangle("fill", obstacle.left - 6, obstacle.top - 7, 14, 14)
    end
    if not playing then
        love.graphics.setColor(0.025, 0.055, 0.09)
        love.graphics.rectangle("fill", 78, 84, 228, 68)
        love.graphics.setColor(0.35, 1, 0.72)
        love.graphics.rectangle("line", 78, 84, 228, 68)
        love.graphics.print("A TO LAUNCH / RETRY", 102, 101)
        love.graphics.print("UP + DOWN TO DODGE", 102, 124)
    end
    love.graphics.setColor(0.65, 0.78, 0.82)
    love.graphics.print("UP/DOWN MOVE   A RETRY   HOME EXIT", 12, 224)
end
