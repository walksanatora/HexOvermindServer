--run this to create the database table we will be using
CREATE TABLE `HexDataStorage` (
    `Pattern` VARCHAR DEFAULT '' COMMENT 'the pattern to lookup db info',
    `Data` MEDIUMBLOB COMMENT 'the NBT data of the object',
    `Key` TINYBLOB COMMENT 'the key to delete this data',
    `Creation` TIMESTAMP DEFAULT NULL COMMENT 'The time when this was create to scheduel destruction',
    PRIMARY KEY (`Pattern`)
);

--delete all objects older then 1 hour
DELETE FROM `HexDataStorage` WHERE `Creation` < NOW() - INTERVAL 1 HOUR;
--pull data with a pattern
SELECT `Data` FROM `HexDataStorage` WHERE `Pattern`  = "known pattern";
--delete with a key
DELETE FROM `HexDataStorage` WHERE `Pattern` = "known pattern" AND `Key` = "";
--add a value into the database
INSERT INTO `HexDataStorage` (Pattern,Data,Key,Creation) VALUES ("qqaw","nbtstuff","255 byte key",NOW());