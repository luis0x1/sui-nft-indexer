/*
  Warnings:

  - The primary key for the `displays` table will be changed. If it partially fails, the table could be left without primary key constraint.
  - The `id` column on the `displays` table would be dropped and recreated. This will lead to data loss if there is data in the column.
  - A unique constraint covering the columns `[object_type]` on the table `displays` will be added. If there are existing duplicate values, this will fail.

*/
-- DropIndex
DROP INDEX "displays_object_type_idx";

-- AlterTable
ALTER TABLE "displays" DROP CONSTRAINT "displays_pkey",
DROP COLUMN "id",
ADD COLUMN     "id" SERIAL NOT NULL,
ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- AlterTable
ALTER TABLE "objects" ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- AlterTable
ALTER TABLE "packages" ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- CreateIndex
CREATE UNIQUE INDEX "displays_object_type_key" ON "displays"("object_type");
